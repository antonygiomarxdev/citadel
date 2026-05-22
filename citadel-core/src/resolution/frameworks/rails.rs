use crate::resolution::{FrameworkResolver, ResolvedRef, ResolutionContext};

pub struct RailsResolver;

impl FrameworkResolver for RailsResolver {
    fn name(&self) -> &str { "rails" }

    fn detect(&self, ctx: &dyn ResolutionContext) -> bool {
        ctx.file_exists("Gemfile") || ctx.file_exists("config/application.rb")
    }

    fn resolve(&self, _ref: &crate::types::UnresolvedRef, _ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        None
    }
}
