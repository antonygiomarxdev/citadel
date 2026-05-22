use crate::resolution::{FrameworkResolver, ResolvedRef, ResolutionContext};
use crate::types::*;

pub struct GinResolver;

impl FrameworkResolver for GinResolver {
    fn name(&self) -> &str { "gin" }

    fn detect(&self, ctx: &dyn ResolutionContext) -> bool {
        ctx.file_exists("go.mod")
            && ctx.read_file("go.mod").is_ok_and(|content| {
                content.is_some_and(|c| c.contains("gin-gonic"))
            })
    }

    fn resolve(&self, _ref: &UnresolvedRef, _ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        None
    }
}
