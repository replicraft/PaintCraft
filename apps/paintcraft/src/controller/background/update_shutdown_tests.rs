use super::*;

#[test]
fn taking_shared_executor_revokes_all_producers_then_retires_original_job_budget() {
    let budget = paintcraft_runtime::SharedBudget::new(PREPARATION_CHARGE);
    let service = Service::with_budget(egui::Context::default(), budget.clone()).unwrap();
    let alias = service.clone();
    let release = service.pause();
    assert_eq!(budget.usage().reserved, PREPARATION_CHARGE);
    let retire = service.retire_for_update().unwrap();
    assert!(
        alias
            .submit(
                |_| Err(Error::new(ErrorCode::OperationCancelled, "must not run")),
                "test".into(),
                "",
                None
            )
            .is_err()
    );
    assert_eq!(budget.usage().reserved, PREPARATION_CHARGE);
    release.send(()).unwrap();
    retire().unwrap();
    assert_eq!(budget.usage().reserved, 0);
    assert!(alias.retire_for_update().unwrap()().is_ok());
}
