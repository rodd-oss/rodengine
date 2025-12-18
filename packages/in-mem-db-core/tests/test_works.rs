use ntest::timeout;

#[timeout(1000)]
#[test]
fn it_works() {
    assert_eq!(2 + 2, 4);
}
