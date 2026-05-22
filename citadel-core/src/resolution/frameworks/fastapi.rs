use crate::resolution::{FrameworkResolver, ResolvedRef, ResolutionContext};

pub struct FastAPIResolver;

impl FrameworkResolver for FastAPIResolver {
    fn name(&self) -> &str { "fastapi" }

    fn detect(&self, ctx: &dyn ResolutionContext) -> bool {
        ctx.file_exists("requirements.txt") || ctx.file_exists("pyproject.toml")
    }

    fn resolve(&self, _ref: &crate::types::UnresolvedRef, _ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        None
    }
}
