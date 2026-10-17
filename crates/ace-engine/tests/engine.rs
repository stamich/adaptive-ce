//! Tests of `ace_engine` `engine` (formerly the inline unit tests of `src`).

use ace_core::AceConfig;
use ace_engine::*;

/// The configuration used by [`AceEngine::default_engine`] passes full validation.
#[test]
fn default_configuration_is_valid() {
    assert!(AceEngine::new(AceConfig::default()).is_ok());
}
