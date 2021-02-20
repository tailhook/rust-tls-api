use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;

use rustls::Session;
use rustls::StreamOwned;

use tls_api::spi::async_as_sync::AsyncIoAsSyncIo;
use tls_api::spi::async_as_sync::AsyncWrapperOps;
use tls_api::spi::async_as_sync::TlsStreamOverSyncIo;
use tls_api::AsyncSocket;
use tls_api::ImplInfo;

pub(crate) type TlsStream<A, T> =
    TlsStreamOverSyncIo<A, AsyncWrapperOpsImpl<T, AsyncIoAsSyncIo<A>, A>>;

#[derive(Debug)]
pub(crate) struct AsyncWrapperOpsImpl<T, S, A>(PhantomData<(T, S, A)>)
where
    S: fmt::Debug + Unpin + Send + 'static,
    A: AsyncSocket,
    T: Session + Sized + fmt::Debug + Unpin + 'static;

#[derive(Debug)]
struct StreamOwnedDebug;

impl<T, S, A> AsyncWrapperOps<A> for AsyncWrapperOpsImpl<T, S, A>
where
    S: fmt::Debug + Unpin + Send + 'static,
    A: AsyncSocket,
    T: Session + Sized + fmt::Debug + Unpin + 'static,
{
    type SyncWrapper = StreamOwned<T, AsyncIoAsSyncIo<A>>;

    fn impl_info() -> ImplInfo {
        crate::info()
    }

    fn debug(_w: &Self::SyncWrapper) -> &dyn Debug {
        // TODO: remove on next release https://github.com/ctz/rustls/pull/524
        &StreamOwnedDebug
    }

    fn get_mut(w: &mut Self::SyncWrapper) -> &mut AsyncIoAsSyncIo<A> {
        w.get_mut()
    }

    fn get_ref(w: &Self::SyncWrapper) -> &AsyncIoAsSyncIo<A> {
        w.get_ref()
    }

    fn get_alpn_protocol(w: &Self::SyncWrapper) -> tls_api::Result<Option<Vec<u8>>> {
        Ok(w.sess.get_alpn_protocol().map(Vec::from))
    }
}
