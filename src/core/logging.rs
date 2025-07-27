pub fn log_format(title: &str, content: &str) -> String {
    format!(
        "\n*** *** *** [{}] *** *** ***\n{}\n*** *** *** [{}] *** *** ***\n\n",
        title, content, title
    )
}

pub trait PolarBearExpectation<T> {
    fn pb_expect(self, msg: &str) -> T;
}

impl<T, E> PolarBearExpectation<T> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn pb_expect(self, msg: &str) -> T {
        self.expect(&log_format("POLAR BEAR EXPECTATION", msg))
    }
}

impl<T> PolarBearExpectation<T> for Option<T> {
    fn pb_expect(self, msg: &str) -> T {
        self.expect(&log_format("POLAR BEAR EXPECTATION", msg))
    }
}
