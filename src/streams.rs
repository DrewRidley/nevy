
trait Streams {
    const COUNT: usize;

    fn id(self) -> usize;

    fn priority(self) -> i32;
}

#[macro_export]
macro_rules! streams {
    {$vis:vis $name:ident {$($variant:ident$(($priority:expr))?),*,}} => {
        #[derive(Clone, Copy)]
        $vis enum $name {
            $($variant),*
        }

        impl Streams for $name {
            const COUNT: usize = [$(Self::$variant),*].len();

            fn id(self) -> usize {
                self as usize
            }

            fn priority(self) -> i32 {
                match self {
                    $($(Self::$variant => $priority)?),*
                    _ => 0
                }
            }
        }
    };
}

