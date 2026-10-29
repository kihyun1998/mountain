fn assert_send<T: Send>() {}

#[test]
fn the_renderer_can_move_to_another_thread() {
    assert_send::<mountain_vello::VelloRenderer>();
}
