use crate::resolution::{FrameworkResolver, ResolvedRef, ResolutionContext};
use crate::types::*;

pub struct ExpressResolver;

impl FrameworkResolver for ExpressResolver {
    fn name(&self) -> &str { "express" }

    fn detect(&self, ctx: &dyn ResolutionContext) -> bool {
        ctx.file_exists("package.json")
    }

    fn resolve(&self, _ref: &UnresolvedRef, _ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        None
    }
}
