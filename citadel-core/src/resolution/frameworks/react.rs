use crate::resolution::{FrameworkResolver, ResolvedRef, ResolutionContext};
use crate::types::*;

pub struct ReactResolver;

impl FrameworkResolver for ReactResolver {
    fn name(&self) -> &str { "react" }

    fn detect(&self, ctx: &dyn ResolutionContext) -> bool {
        ctx.file_exists("package.json")
    }

    fn resolve(&self, _ref: &UnresolvedRef, _ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        None
    }
}
