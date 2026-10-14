//! Separate process from `logging.rs`, so this is the first logger installed here.

#[test]
fn the_default_logger_installs_only_when_none_exists_and_does_not_replace_the_filter() {
    engine::install_stderr_logger();
    assert_eq!(log::max_level(), log::LevelFilter::Info);
    log::set_max_level(log::LevelFilter::Warn);
    engine::install_stderr_logger();
    assert_eq!(log::max_level(), log::LevelFilter::Warn);
}
