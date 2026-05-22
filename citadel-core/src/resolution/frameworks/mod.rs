pub mod express;
pub mod react;
pub mod nestjs;
pub mod django;
pub mod flask;
pub mod fastapi;
pub mod rails;
pub mod gin;
pub mod go_stdlib;

use crate::resolution::FrameworkResolver;

/// Returns all built-in framework resolvers.
pub fn all_frameworks() -> Vec<Box<dyn FrameworkResolver>> {
    vec![
        Box::new(express::ExpressResolver),
        Box::new(react::ReactResolver),
        Box::new(nestjs::NestJSResolver),
        Box::new(django::DjangoResolver),
        Box::new(flask::FlaskResolver),
        Box::new(fastapi::FastAPIResolver),
        Box::new(rails::RailsResolver),
        Box::new(gin::GinResolver),
        Box::new(go_stdlib::GoStdlibResolver),
    ]
}
