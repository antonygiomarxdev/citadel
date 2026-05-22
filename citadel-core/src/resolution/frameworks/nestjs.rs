use crate::resolution::{FrameworkResolver, ResolvedRef, ResolutionContext};
use crate::types::*;

pub struct NestJSResolver;

impl FrameworkResolver for NestJSResolver {
    fn name(&self) -> &str { "nestjs" }

    fn detect(&self, ctx: &dyn ResolutionContext) -> bool {
        ctx.file_exists("package.json")
            && ctx.read_file("package.json").is_ok_and(|content| {
                content.is_some_and(|c| c.contains("@nestjs"))
            })
    }

    fn resolve(&self, _ref: &UnresolvedRef, _ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        None
    }
}
