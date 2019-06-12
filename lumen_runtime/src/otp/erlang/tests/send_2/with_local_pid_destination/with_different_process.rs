use super::*;

#[test]
fn with_locked_adds_heap_message_to_mailbox_and_returns_message() {
    with_process_arc(|arc_process| {
        TestRunner::new(Config::with_source_file(file!()))
            .run(&strategy::term(arc_process.clone()), |message| {
                let destination = arc_process.pid;

                prop_assert_eq!(
                    erlang::send_2(destination, message, &arc_process),
                    Ok(message)
                );

                prop_assert!(has_process_message(&arc_process, message));

                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn without_locked_adds_process_message_to_mailbox_and_returns_message() {
    with_process_arc(|arc_process| {
        TestRunner::new(Config::with_source_file(file!()))
            .run(
                &strategy::term::can_be_passed_to_different_process(arc_process.clone()),
                |message| {
                    let different_arc_process = process::local::test(&arc_process);
                    let destination = different_arc_process.pid;

                    prop_assert_eq!(
                        erlang::send_2(destination, message, &arc_process),
                        Ok(message)
                    );

                    prop_assert!(has_process_message(&different_arc_process, message));

                    Ok(())
                },
            )
            .unwrap();
    });
}
