/// Allows to call boxed closures
///
/// details about this helper can be found on the rust book chapter 20: building a
/// multi-threaded web server
pub trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}
