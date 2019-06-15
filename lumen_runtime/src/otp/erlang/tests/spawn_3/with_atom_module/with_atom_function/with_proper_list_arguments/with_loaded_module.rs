use super::*;

mod with_exported_function;

#[test]
fn without_exported_function_when_run_exits_undef() {
    with_process(|parent_process| {
        let arc_scheduler = Scheduler::current();

        let priority = Priority::Normal;
        let run_queue_length_before = arc_scheduler.run_queue_len(priority);

        let module = Term::str_to_atom("erlang", DoNotCare).unwrap();
        // Rust name instead of Erlang name
        let function = Term::str_to_atom("number_or_badarith_1", DoNotCare).unwrap();
        let arguments = Term::cons(
            0.into_process(parent_process),
            Term::EMPTY_LIST,
            parent_process,
        );

        let result = erlang::spawn_3(module, function, arguments, parent_process);

        assert!(result.is_ok());

        let child_pid = result.unwrap();

        assert_eq!(child_pid.tag(), LocalPid);

        let run_queue_length_after = arc_scheduler.run_queue_len(priority);

        assert_eq!(run_queue_length_after, run_queue_length_before + 1);

        let arc_process = pid_to_process(child_pid).unwrap();

        arc_scheduler.run_through(&arc_process);

        assert_eq!(arc_process.stack_len(), 1);
        assert_eq!(
            arc_process.current_module_function_arity(),
            Some(Arc::new(ModuleFunctionArity {
                module,
                function,
                arity: 1
            }))
        );

        match *arc_process.status.read().unwrap() {
            Status::Exiting(ref exception) => {
                assert_eq!(
                    exception,
                    &undef!(module, function, arguments, &arc_process)
                );
            }
            ref status => panic!("Process status ({:?}) is not exiting.", status),
        };
    });
}