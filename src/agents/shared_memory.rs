use {
    autoagents::{
        async_trait,
        core::agent::memory::{MemoryProvider, MemoryType, SlidingWindowMemory},
        llm::chat::ChatMessage,
        llm_error::LLMError,
    },
    bevy::ecs::resource::Resource,
    std::sync::Arc,
    tokio::sync::Mutex,
};

const SLIDING_WINDOW_MEMORY_SIZE: usize = 300;

#[derive(Clone, Resource)]
pub struct SharedSlidingWindowMemory {
    inner: Arc<Mutex<SlidingWindowMemory>>,
}

#[allow(dead_code)]
impl SharedSlidingWindowMemory {
    pub fn new(window_size: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SlidingWindowMemory::new(window_size))),
        }
    }

    pub fn window_size(&self) -> usize {
        tokio::task::block_in_place::<_, usize>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.window_size() })
        })
    }

    pub fn messages(&self) -> Vec<ChatMessage> {
        tokio::task::block_in_place::<_, Vec<ChatMessage>>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.messages() })
        })
    }

    pub fn recent_messages(&self, limit: usize) -> Vec<ChatMessage> {
        tokio::task::block_in_place::<_, Vec<ChatMessage>>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.recent_messages(limit) })
        })
    }

    pub fn needs_summary(&self) -> bool {
        tokio::task::block_in_place::<_, bool>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.needs_summary() })
        })
    }

    pub fn mark_for_summary(&mut self) {
        () = tokio::task::block_in_place::<_, ()>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.mark_for_summary() })
        });
    }

    pub fn replace_with_summary(&mut self, summary: String) {
        () = tokio::task::block_in_place::<_, ()>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.replace_with_summary(summary) })
        });
    }
}

#[async_trait]
impl MemoryProvider for SharedSlidingWindowMemory {
    async fn remember(&mut self, message: &ChatMessage) -> Result<(), LLMError> {
        tokio::task::block_in_place::<_, Result<(), LLMError>>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.remember(message).await })
        })
    }

    async fn recall(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<ChatMessage>, LLMError> {
        tokio::task::block_in_place::<_, Result<Vec<ChatMessage>, LLMError>>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.recall(query, limit).await })
        })
    }

    async fn clear(&mut self) -> Result<(), LLMError> {
        tokio::task::block_in_place::<_, Result<(), LLMError>>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.clear().await })
        })
    }

    fn memory_type(&self) -> MemoryType {
        MemoryType::Custom
    }

    fn size(&self) -> usize {
        tokio::task::block_in_place::<_, usize>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.size() })
        })
    }

    fn clone_box(&self) -> Box<dyn MemoryProvider> {
        Box::new(self.clone())
    }

    fn needs_summary(&self) -> bool {
        tokio::task::block_in_place::<_, bool>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.needs_summary() })
        })
    }

    fn mark_for_summary(&mut self) {
        () = tokio::task::block_in_place::<_, ()>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.mark_for_summary() })
        });
    }

    fn replace_with_summary(&mut self, summary: String) {
        () = tokio::task::block_in_place::<_, ()>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.replace_with_summary(summary) })
        });
    }

    fn preload(&mut self, data: Vec<ChatMessage>) -> bool {
        tokio::task::block_in_place::<_, bool>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.preload(data) })
        })
    }

    fn export(&self) -> Vec<ChatMessage> {
        tokio::task::block_in_place::<_, Vec<ChatMessage>>(|| {
            tokio::runtime::Handle::current()
                .block_on::<_>(async { self.inner.lock().await.export() })
        })
    }
}

impl Default for SharedSlidingWindowMemory {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SlidingWindowMemory::new(
                SLIDING_WINDOW_MEMORY_SIZE,
            ))),
        }
    }
}
