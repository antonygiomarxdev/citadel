use crate::resolution::{FrameworkResolver, ResolvedRef, ResolutionContext};
use crate::types::*;

pub struct DjangoResolver;

impl FrameworkResolver for DjangoResolver {
    fn name(&self) -> &str { "django" }

    fn detect(&self, ctx: &dyn ResolutionContext) -> bool {
        ctx.file_exists("manage.py") || ctx.file_exists("requirements.txt")
    }

    fn resolve(&self, _ref: &UnresolvedRef, _ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        None
    }
}
