pub mod kokoro;

#[async_trait::async_trait]
pub trait TTSEngine: Send + Sync {
    async fn speak(&mut self, text: &str, speed: f32, volume: f32) -> anyhow::Result<()>;
    async fn stop(&self) -> anyhow::Result<()>;
}
