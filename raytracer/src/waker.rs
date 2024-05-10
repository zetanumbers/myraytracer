use std::{sync::Arc, task};

use winit::event_loop::EventLoopProxy;

use crate::AppEvent;

pub type AppEventDispatchWaker = EventDispatchWaker<AppEvent>;

#[derive(Clone)]
pub struct EventDispatchWaker<T: 'static> {
    dispatch: EventLoopProxy<T>,
    event: T,
}

impl<T: 'static> EventDispatchWaker<T> {
    pub fn new(dispatch: EventLoopProxy<T>, event: T) -> Self {
        Self { dispatch, event }
    }

    pub fn into_waker(self) -> task::Waker
    where
        T: Clone + Send + Sync,
    {
        Arc::new(self).into()
    }
}

impl<T> From<EventDispatchWaker<T>> for task::Waker
where
    T: Clone + Send + Sync + 'static,
{
    #[inline(always)]
    fn from(value: EventDispatchWaker<T>) -> Self {
        value.into_waker()
    }
}

impl<T> task::Wake for EventDispatchWaker<T>
where
    T: Clone + 'static,
{
    fn wake(self: Arc<Self>) {
        self.wake_by_ref()
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let _ = self.dispatch.send_event(self.event.clone());
    }
}
