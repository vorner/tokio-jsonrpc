// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! The endpoint of the JSON RPC connection.
//!
//! This module helps building the endpoints of the connection. The endpoints act as both client
//! and server at the same time. If you want a client-only endpoint, use
//! [`Empty`](../server/struct.Empty.html) as the server or the relevant
//! [`Endpoint`](struct.Endpoint.html)'s constructor. If you want a server-only endpoint,
//! simply don't call any RPCs or notifications and forget about the returned
//! [`Client`](struct.Client.html) structure.

use std::io::{self, Error as IoError, ErrorKind};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use std::cell::RefCell;

use futures::{Future, IntoFuture, Stream, Sink};
use futures::future::Either;
use futures::stream::{self, Once, empty, unfold};
use futures_mpsc::{channel, Sender};
use relay::{channel as relay_channel, Sender as RelaySender};
use serde_json::{Value, to_value};
use tokio_core::reactor::{Handle, Timeout};

use message::{Broken, Message, Parsed, Response, Request, RPCError, Notification};
use server::{Empty as EmptyServer, Server};

/// Thing that terminates the connection once dropped.
///
/// A trick to terminate when all Rcs are forgotten.
struct DropTerminator(Option<RelaySender<()>>);

impl Drop for DropTerminator {
    fn drop(&mut self) {
        self.0.take().unwrap().complete(());
    }
}

type RcDrop = Rc<DropTerminator>;

/// An internal part of `ServerCtl`.
///
/// The `ServerCtl` is just a thin Rc wrapper around this.
struct ServerCtlInternal {
    // Stop processing requests
    stop: bool,
    // Terminate the nice way (if all others also drop)
    terminator: Option<RcDrop>,
    // Terminate right now
    killer: Option<RelaySender<()>>,
    // Info to be able to create a new clients
    idmap: IDMap,
    handle: Handle,
    sender: Option<Sender<Message>>,
}

/// A handle to control the server.
///
/// An instance is provided to each [`Server`](../server/trait.Server.html) callback and it can be
/// used to manipulate the server (currently only to terminate the server) or to create a client
/// for the use of the server.
#[derive(Clone)]
pub struct ServerCtl(Rc<RefCell<ServerCtlInternal>>);

impl ServerCtl {
    /// Perform a cleanup when terminating in some way.
    ///
    /// And perform some operation on the internal (access for convenience)
    fn cleanup<R, F: FnOnce(&mut ServerCtlInternal) -> R>(&self, f: F) -> R {
        let mut internal = self.0.borrow_mut();
        internal.stop = true;
        internal.sender.take();
        f(&mut internal)
    }
    /// Stop answering RPCs and calling notifications.
    ///
    /// Also terminate the connection if the client handle has been dropped and all ongoing RPC
    /// answers were received.
    pub fn terminate(&self) {
        self.cleanup(|internal| {
            // Drop the reference count for this one
            internal.terminator.take();
        });
    }
    /// Kill the connection.
    ///
    /// Like, right now. Without a goodbye.
    pub fn kill(&self) {
        self.cleanup(|internal| {
            // The option might be None, but only after we called it already.
            internal.killer.take().map(|s| s.complete(()));
        });
    }
    /// Create a new client for the current endpoint.
    ///
    /// This is a way in which the server may access the other endpoint (eg. call RPCs or send
    /// notifications to the other side).
    ///
    /// # Panics
    ///
    /// If called after `kill` or `terminate` has been called previously.
    pub fn client(&self) -> Client {
        let internal = self.0.borrow();
        Client::new(&internal.idmap,
                    self,
                    &internal.handle,
                    internal.terminator.as_ref().expect("`client` called after termination`"),
                    internal.sender.as_ref().expect("`client` called after termination"))
    }
}

// Our own BoxFuture & friends that is *not* send. We don't do send.
type BoxFuture<T, E> = Box<Future<Item = T, Error = E>>;
type FutureMessage = BoxFuture<Option<Message>, IoError>;
type BoxStream<T, E> = Box<Stream<Item = T, Error = E>>;
// None in error means end the stream, please
type FutureMessageStream = BoxStream<FutureMessage, IoError>;

type IDMap = Rc<RefCell<HashMap<String, RelaySender<Response>>>>;

// A future::stream::once that takes only the success value, for convenience.
fn once<T, E>(item: T) -> Once<T, E> {
    stream::once(Ok(item))
}

fn shouldnt_happen<E>(_: E) -> IoError {
    IoError::new(ErrorKind::Other, "Shouldn't happen")
}

fn do_request<RPCServer: Server + 'static>(server: &RPCServer, ctl: &ServerCtl, request: Request) -> FutureMessage {
    match server.rpc(ctl, &request.method, &request.params) {
        None => Box::new(Ok(Some(request.error(RPCError::method_not_found(request.method.clone())))).into_future()),
        Some(future) => {
            Box::new(future.into_future().then(move |result| match result {
                Err(err) => Ok(Some(request.error(err))),
                Ok(result) => Ok(Some(request.reply(to_value(result).expect("Trying to return a value that can't be converted to JSON")))),
            }))
        },
    }
}

fn do_notification<RPCServer: Server>(server: &RPCServer, ctl: &ServerCtl, notification: &Notification) -> FutureMessage {
    match server.notification(ctl, &notification.method, &notification.params) {
        None => Box::new(Ok(None).into_future()),
        // We ignore both success and error, so we convert it into something for now
        Some(future) => Box::new(future.into_future().then(|_| Ok(None))),
    }
}

// To process a batch using the same set of parallel executors as the whole server, we produce a
// stream of the computations which return nothing, but gather the results. Then we add yet another
// future at the end of that stream that takes the gathered results and wraps them into the real
// message ‒ the result of the whole batch.
fn do_batch<RPCServer: Server + 'static>(server: &RPCServer, ctl: &ServerCtl, idmap: &IDMap, msg: Vec<Message>) -> FutureMessageStream {
    // Create a large enough channel. We may be unable to pick up the results until the final
    // future gets its turn, so shorter one could lead to a deadlock.
    let (sender, receiver) = channel(msg.len());
    // Each message produces a single stream of futures. Create that streams (right now, so we
    // don't have to keep server long into the future).
    let small_streams: Vec<_> = msg.into_iter()
        .map(|sub| -> Result<_, IoError> {
            let sender = sender.clone();
            // This part is a bit convoluted. The do_msg returns a stream of futures. We want to
            // take each of these futures (the outer and_then), run it to completion (the inner
            // and_then), send its result through the sender if it provided one and then
            // convert the result to None.
            //
            // Note that do_msg may return arbitrary number of work futures, but only at most one
            // of them is supposed to provide a resulting value which would be sent through the
            // channel. Unfortunately, there's no way to know which one it'll be, so we have to
            // clone the sender all over the place.
            //
            // Also, it is a bit unfortunate how we need to allocate so many times here. We may try
            // doing something about that in the future, but without implementing custom future and
            // stream types, this seems the best we can do.
            let all_sent = do_msg(server, ctl, idmap, Ok(sub)).and_then(move |future_message| -> Result<FutureMessage, _> {
                let sender = sender.clone();
                let msg_sent = future_message.and_then(move |response: Option<Message>| match response {
                    None => Either::A(Ok(None).into_future()),
                    Some(msg) => {
                        Either::B(sender.send(msg)
                            .map_err(shouldnt_happen)
                            .map(|_| None))
                    },
                });
                Ok(Box::new(msg_sent))
            });
            Ok(all_sent)
        })
        .collect();
    // We make it into a stream of streams and flatten it to get one big stream
    let subs_stream = stream::iter(small_streams).flatten();
    // Once all the results are produced, wrap them into a batch and return that one
    let collected = receiver.collect()
        .map_err(shouldnt_happen)
        .map(|results| if results.is_empty() {
            // The spec says to send nothing at all if there are no results
            None
        } else {
            Some(Message::Batch(results))
        });
    let streamed: Once<FutureMessage, _> = once(Box::new(collected));
    // Connect the wrapping single-item stream after the work stream
    Box::new(subs_stream.chain(streamed))
}

fn do_response(idmap: &IDMap, response: Response) -> FutureMessageStream {
    let maybe_sender = response.id
        .as_str()
        .and_then(|id| idmap.borrow_mut().remove(id));
    if let Some(sender) = maybe_sender {
        sender.complete(response);
    }
    // TODO: Else ‒ some logging
    Box::new(empty())
}

// Handle single message and turn it into an arbitrary number of futures that may be worked on in
// parallel, but only at most one of which returns a response message
fn do_msg<RPCServer: Server + 'static>(server: &RPCServer, ctl: &ServerCtl, idmap: &IDMap, msg: Parsed) -> FutureMessageStream {
    let terminated = ctl.0.borrow().stop;
    if terminated {
        if let Ok(Message::Response(response)) = msg {
            do_response(idmap, response);
        }
        Box::new(empty())
    } else {
        match msg {
            Err(broken) => {
                let err: FutureMessage = Ok(Some(broken.reply())).into_future().boxed();
                Box::new(once(err))
            },
            Ok(Message::Request(req)) => Box::new(once(do_request(server, ctl, req))),
            Ok(Message::Notification(notif)) => Box::new(once(do_notification(server, ctl, &notif))),
            Ok(Message::Batch(batch)) => do_batch(server, ctl, idmap, batch),
            Ok(Message::UnmatchedSub(value)) => do_msg(server, ctl, idmap, Err(Broken::Unmatched(value))),
            Ok(Message::Response(response)) => do_response(idmap, response),
        }
    }
}

/// Internal part of the client.
///
/// Just for convenience, as we need to deconstruct and construct it repeatedly, so this way we
/// have only one item extra.
#[derive(Clone)]
struct ClientData {
    /// Mapping from IDs to the relays to wake up the recipient futures.
    idmap: IDMap,
    /// The control of the server.
    ctl: ServerCtl,
    handle: Handle,
    /// Keep the connection alive as long as the client is alive.
    terminator: RcDrop,
}

/// The client part of the endpoint.
///
/// This can be used to call RPCs and send notifications to the other end. There's no direct
/// constructor, it is created through the [Endpoint](struct.Endpoint.html).
#[derive(Clone)]
pub struct Client {
    sender: Sender<Message>,
    data: ClientData,
}

pub type Notified = BoxFuture<Client, IoError>;
pub type RPCFinished = BoxFuture<Option<Response>, IoError>;
pub type RPCSent = BoxFuture<(Client, RPCFinished), IoError>;

impl Client {
    /// A constructor (a private one).
    fn new(idmap: &IDMap, ctl: &ServerCtl, handle: &Handle, terminator: &RcDrop, sender: &Sender<Message>) -> Self {
        Client {
            sender: sender.clone(),
            data: ClientData {
                idmap: idmap.clone(),
                ctl: ctl.clone(),
                handle: handle.clone(),
                terminator: terminator.clone(),
            },
        }
    }
    /// Call a RPC.
    ///
    /// Construct an RPC message and send it to the other end. It returns a future that resolves
    /// once the message is sent. It yields the Client back (it is blocked for the time of sending)
    /// and another future that resolves once the answer is received (or once a timeout happens, in
    /// which case the result is None).
    pub fn call(self, method: String, params: Option<Value>, timeout: Option<Duration>) -> RPCSent {
        // We have to deconstruct self now, because the sender's send takes ownership for it for a
        // while. We construct it back once the message is passed on.
        let data = self.data;
        let msg = Message::request(method, params);
        let id = match msg {
            Message::Request(Request { id: Value::String(ref id), .. }) => id.clone(),
            _ => unreachable!("We produce only string IDs"),
        };
        let (sender, receiver) = relay_channel();
        let rc_terminator = data.terminator.clone();
        let received = receiver.map_err(|_| IoError::new(io::ErrorKind::Other, "Lost connection"))
            .map(Some)
            .then(move |r| {
                drop(rc_terminator);
                r
            });
        let completed: RPCFinished = match timeout {
            Some(time) => {
                // If we were provided with a timeout, select what happens first.
                let timeout = match Timeout::new(time, &data.handle) {
                    Err(e) => return Box::new(Err(e).into_future()),
                    Ok(t) => t,
                };
                let idmap = data.idmap.clone();
                let id = id.clone();
                let completed = timeout
                    .map(|_| None)
                    .select(received)
                    .map(|(r, _)| r)
                    .map_err(|(e, _)| e)
                    // Make sure the ID/sender is removed even when timeout wins.
                    // This is a NOOP in case the real result arrives, since it is already deleted
                    // by then, but that doesn't matter and this is simpler.
                    .then(move |r| {
                        idmap.borrow_mut().remove(&id);
                        r
                    });
                Box::new(completed)
            },
            // If we don't have the timeout, simply pass the future to get the response through.
            None => Box::new(received),
        };
        data.idmap.borrow_mut().insert(id, sender);
        // Ensure the connection is kept alive until the answer comes
        let sent = self.sender
            .send(msg)
            .map_err(shouldnt_happen)
            .map(move |sender| {
                let client = Client {
                    sender: sender,
                    data: data,
                };
                (client, completed)
            });
        Box::new(sent)
    }
    /// Send a notification.
    ///
    /// It creates a notification message and sends it. It returs a future that resolves once the
    /// message is sent and yields the client back for further use.
    pub fn notify(self, method: String, params: Option<Value>) -> Notified {
        let data = self.data;
        let future = self.sender
            .send(Message::notification(method, params))
            .map_err(shouldnt_happen)
            .map(move |sender| {
                Client {
                    sender: sender,
                    data: data,
                }
            });
        Box::new(future)
    }
    /// Get the server control.
    ///
    /// That allows terminating the server, etc.
    pub fn server_ctl(&self) -> &ServerCtl {
        &self.data.ctl
    }
}

/// The builder structure for the end point.
///
/// This is used to create the endpoint ‒ both the server and client part at once.
///
/// # Examples
///
/// This will create a connection, build a client-only endpoint (eg. the server is dummy) on it and
/// send an RPC to the other side, printing the result once it comes.
///
/// ```rust,no_run
/// # extern crate tokio_core;
/// # extern crate tokio_jsonrpc;
/// # extern crate futures;
/// # #[macro_use]
/// # extern crate serde_json;
/// #
/// # use std::time::Duration;
/// # use tokio_core::reactor::Core;
/// # use tokio_core::net::TcpStream;
/// # use tokio_core::io::Io;
/// # use tokio_jsonrpc::{LineCodec, Server, ServerCtl, RPCError, Endpoint};
/// # use tokio_jsonrpc::message::Response;
/// # use futures::{Future, Stream};
/// # use serde_json::Value;
/// #
/// # fn main() {
/// let mut core = Core::new().unwrap();
/// let handle = core.handle();
///
/// let request = TcpStream::connect(&"127.0.0.1:2346".parse().unwrap(), &handle)
///     .map(move |stream| {
///         // Create a client on top of the connection
///         let (client, _finished) = Endpoint::client_only(stream.framed(LineCodec::new()))
///             .start(&handle);
///         // Call a method with some parameters and a 10 seconds timeout
///         client.call("request".to_owned(),
///                     Some(json!(["param1", "param2"])),
///                     Some(Duration::new(10, 0)))
///             .and_then(|(_client, future_result)| future_result)
///             .map(|response| {
///                 match response {
///                     None => println!("A timeout happened"),
///                     Some(Response { result, .. }) => println!("The answer is {:?}", result),
///                 }
///             })
///     });
///
/// core.run(request).unwrap();
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct Endpoint<Connection, RPCServer> {
    connection: Connection,
    server: RPCServer,
    parallel: usize,
}

impl<Connection, RPCServer> Endpoint<Connection, RPCServer>
    where
        Connection: Stream<Item = Parsed, Error = IoError>,
        Connection: Sink<SinkItem = Message, SinkError = IoError>,
        Connection: Send + 'static,
        RPCServer: Server + 'static {
    /// Create the endpoint builder.
    ///
    /// Pass it the connection to build the endpoint on and the server to use internally.
    pub fn new(connection: Connection, server: RPCServer) -> Self {
        Endpoint {
            connection: connection,
            server: server,
            parallel: 1,
        }
    }
    /// Set how many RPCs may be process in parallel.
    ///
    /// As the RPC may be a future, it is possible to have multiple of them in the pipeline at
    /// once. By default, no parallelism is allowed on one endpoint. Calling this sets how many
    /// parallel futures there may be at one time on this particular endpoint.
    pub fn parallel(self, parallel: usize) -> Self {
        Endpoint { parallel: parallel, ..self }
    }
    /// Start the endpoint.
    ///
    /// Once all configuration is set, this creates the actual endpoint pair ‒ both the server and
    /// the client. It returns the client and a future that resolves once the server terminates.
    /// The future yields if there was any error running the server. The server is started on the
    /// provided handle. It can be manipulated by the [`ServerCtl`](struct.ServerCtl.html), which
    /// is accessible through the returned [`Client`](struct.Client.html) and is passed to each
    /// [`Server`](../server/trait.Server.html) callback.
    // TODO: Description how this works.
    // TODO: Some cleanup. This looks a *bit* hairy and complex.
    // TODO: Should we return a better error/return the error once thing resolves?
    pub fn start(self, handle: &Handle) -> (Client, Box<Future<Item = (), Error = IoError>>) {
        let (terminator_sender, terminator_receiver) = relay_channel();
        let (killer_sender, killer_receiver) = relay_channel();
        let (sender, receiver) = channel(32);
        let idmap = Rc::new(RefCell::new(HashMap::new()));
        let rc_terminator = Rc::new(DropTerminator(Some(terminator_sender)));
        let ctl = ServerCtl(Rc::new(RefCell::new(ServerCtlInternal {
            stop: false,
            terminator: Some(rc_terminator.clone()),
            killer: Some(killer_sender),
            idmap: idmap.clone(),
            handle: handle.clone(),
            sender: Some(sender.clone()),
        })));
        let client = Client::new(&idmap, &ctl, handle, &rc_terminator, &sender);
        let (sink, stream) = self.connection.split();
        // Create a future for each received item that'll return something. Run some of them in
        // parallel.

        // TODO: Have a concrete enum-type for the futures so we don't have to allocate and box it.

        // A trick to terminate when the receiver fires. We mix a None into the stream from another
        // stream and stop when we find it, as a marker.
        let terminator = terminator_receiver.map(|_| None)
            .map_err(shouldnt_happen)
            .into_stream();
        // Move out of self, otherwise the closure captures self, not only server :-|
        let server = self.server;
        server.initialized(&ctl);
        let idmap_cloned = idmap.clone();
        // A stream that contains no elements, but cleans the idmap once called (to kill the RPC
        // futures)
        let cleaner = unfold((), move |_| -> Option<Result<_, _>> {
            idmap_cloned.borrow_mut().clear();
            None
        });
        let idmap_cloned = idmap.clone();
        let answers = stream.map(Some)
            .chain(cleaner)
            .select(terminator)
            .take_while(|m| Ok(m.is_some()))
            .map(move |parsed| do_msg(&server, &ctl, &idmap, parsed.unwrap()))
            .flatten()
            .buffer_unordered(self.parallel)
            .filter_map(|message| message);
        // Take both the client RPCs and the answers
        let outbound = answers.select(receiver.map_err(shouldnt_happen));
        let (error_sender, error_receiver) = relay_channel::<Option<IoError>>();
        // And send them all (or kill it, if it happens first)
        let transmitted = sink.send_all(outbound)
            .map(|_| ())
            .select(killer_receiver.map_err(shouldnt_happen))
            .then(move |result| {
                // This will hopefully kill the RPC futures
                // We kill on both ends, because we may kill the connection or the other side may.
                idmap_cloned.borrow_mut().clear();
                match result {
                    Ok(_) => {
                        error_sender.complete(None);
                        Ok(())
                    },
                    Err((e, _select_next)) => {
                        error_sender.complete(Some(e));
                        Err(())
                    },
                }
            });
        // Once the last thing is sent, we're done
        handle.spawn(transmitted);
        let finished_errors = error_receiver.map_err(shouldnt_happen)
            .and_then(|maybe_error| match maybe_error {
                None => Ok(()),
                Some(e) => Err(e),
            });
        (client, Box::new(finished_errors))
    }
}

impl<Connection> Endpoint<Connection, EmptyServer>
    where
        Connection: Stream<Item = Parsed, Error = IoError>,
        Connection: Sink<SinkItem = Message, SinkError = IoError>,
        Connection: Send + 'static {
    /// Create an endpoint with [`Empty`](../server/struct.Empty.html).
    ///
    /// If you want to have client only, you can use this instead of `new`.
    pub fn client_only(connection: Connection) -> Self {
        Self::new(connection, EmptyServer)
    }
}
