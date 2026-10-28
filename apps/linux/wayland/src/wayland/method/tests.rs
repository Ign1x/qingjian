//! 输入框状态的生效边界与过期选区判断。

use super::*;

fn done(method: &mut Method) -> Vec<Input> {
    method.done()
}

/// 激活、更新、换输入框、失活：代次只在进出时变，`done` 一次计一次。
#[test]
fn transitions_follow_done() {
    let mut method = Method::default();
    method.pending.active = Some(true);
    assert_eq!(done(&mut method), [Input::Activate(Field::default())]);
    assert_eq!((method.epoch, method.dones), (1, 1));

    method.pending.external_change = true;
    assert_eq!(
        done(&mut method),
        [Input::Update(Field {
            external_change: true,
            ..Field::default()
        })]
    );
    assert_eq!(method.epoch, 1, "更新不换代次");

    method.pending.active = Some(true);
    assert_eq!(
        done(&mut method),
        [Input::Deactivate, Input::Activate(Field::default())]
    );
    assert_eq!(method.epoch, 3);

    method.pending.active = Some(false);
    assert_eq!(done(&mut method), [Input::Deactivate]);
    assert!(done(&mut method).is_empty(), "失活着不报更新");
    assert_eq!((method.epoch, method.dones), (4, 5));
}

/// 内容类型一直有效到下次改；前后文与变化原因每次 `done` 之后回到初值。
#[test]
fn content_type_persists_but_surrounding_does_not() {
    let mut method = Method {
        password: true,
        ..Method::default()
    };
    method.pending.active = Some(true);
    method.pending.surrounding = Some(Surrounding {
        text: "a".to_owned(),
        cursor: 1,
        anchor: 1,
    });
    let inputs = done(&mut method);
    assert!(
        matches!(&inputs[..], [Input::Activate(field)] if field.password && field.surrounding.is_some())
    );
    let inputs = done(&mut method);
    assert!(
        matches!(&inputs[..], [Input::Update(field)] if field.password && field.surrounding.is_none())
    );
}

#[test]
fn privacy_and_external_edits_invalidate_in_flight_replies() {
    let mut method = Method::default();
    method.pending.active = Some(true);
    method.pending.surrounding = Some(Surrounding {
        text: "苹果".to_owned(),
        cursor: 6,
        anchor: 0,
    });
    done(&mut method);
    let original = method.field.clone();
    assert!(method.selection_current(&original));
    let epoch = method.epoch;

    method.pending.surrounding = original.surrounding.clone();
    method.sensitive = true;
    assert_eq!(method.epoch, epoch, "done 前仍用旧状态");
    assert!(method.selection_current(&original));
    done(&mut method);
    assert_eq!(method.epoch, epoch + 1);
    assert!(!method.selection_current(&original));

    method.pending.surrounding = original.surrounding.clone();
    method.pending.external_change = true;
    done(&mut method);
    assert_eq!(method.epoch, epoch + 1, "内容没变，不重复失效");

    method.pending.surrounding = Some(Surrounding {
        cursor: 0,
        anchor: 0,
        ..original.surrounding.unwrap()
    });
    method.pending.external_change = true;
    done(&mut method);
    assert_eq!(method.epoch, epoch + 2);
}

#[test]
fn input_method_changes_only_invalidate_the_selection_snapshot() {
    let mut method = Method::default();
    method.pending.active = Some(true);
    method.pending.surrounding = Some(Surrounding {
        text: "苹果".to_owned(),
        cursor: 6,
        anchor: 0,
    });
    done(&mut method);
    let original = method.field.clone();
    let epoch = method.epoch;
    method.pending.surrounding = Some(Surrounding {
        cursor: 0,
        anchor: 0,
        ..original.surrounding.clone().unwrap()
    });
    done(&mut method);
    assert_eq!(method.epoch, epoch, "输入法上屏的回执不能丢掉下一键的回包");
    assert!(!method.selection_current(&original));
}
