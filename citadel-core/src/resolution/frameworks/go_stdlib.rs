use crate::resolution::{FrameworkResolver, ResolvedRef, ResolutionContext};
use crate::types::*;

pub struct GoStdlibResolver;

impl FrameworkResolver for GoStdlibResolver {
    fn name(&self) -> &str { "go-stdlib" }

    fn detect(&self, ctx: &dyn ResolutionContext) -> bool {
        ctx.file_exists("go.mod")
    }

    fn resolve(&self, _ref: &UnresolvedRef, _ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        None
    }
}
