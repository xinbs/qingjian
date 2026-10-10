//! 四则运算求值：`+ - * x / % ^`、括号、小数、一元负号。递归下降，结果按整数或最多十位小数输出。

/// 算式求值。算不了（语法错、除零、溢出）返回 `None`。
pub fn evaluate(text: &str) -> Option<String> {
    if text.len() > 128 {
        return None;
    }
    let mut evaluator = Evaluator {
        bytes: text.as_bytes(),
        position: 0,
        depth: 0,
    };
    let value = evaluator.expression()?;
    if evaluator.position != evaluator.bytes.len() || !value.is_finite() {
        return None;
    }
    Some(format_number(value))
}

/// 递归下降求值器，只在 [`evaluate`] 里用。
struct Evaluator<'a> {
    /// 算式文本。
    bytes: &'a [u8],

    /// 当前读到的位置。
    position: usize,

    /// 递归深度上限防止括号、一元符号与乘方链耗尽栈。
    depth: usize,
}

impl Evaluator<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    /// 加减。
    fn expression(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.position += 1;
            let rhs = self.term()?;
            value = if op == b'+' { value + rhs } else { value - rhs };
        }
        Some(value)
    }

    /// 乘除。`x` 也当乘号，键盘上打 `*` 要按 Shift。
    fn term(&mut self) -> Option<f64> {
        let mut value = self.unary()?;
        while let Some(op @ (b'*' | b'x' | b'/' | b'%')) = self.peek() {
            self.position += 1;
            let rhs = self.unary()?;
            value = if matches!(op, b'/' | b'%') {
                if rhs == 0.0 {
                    return None;
                }
                if op == b'/' { value / rhs } else { value % rhs }
            } else {
                value * rhs
            };
        }
        Some(value)
    }

    /// 乘方右结合，优先于一元符号；指数仍可带正负号。
    fn power(&mut self) -> Option<f64> {
        let base = self.atom()?;
        if self.peek() == Some(b'^') {
            self.position += 1;
            let exponent = self.unary()?;
            return Some(base.powf(exponent));
        }
        Some(base)
    }

    /// 每条递归路径经过这里统一计数。
    fn unary(&mut self) -> Option<f64> {
        if self.depth >= 32 {
            return None;
        }
        self.depth += 1;
        let value = match self.peek() {
            Some(b'-') => {
                self.position += 1;
                self.unary().map(|v| -v)
            }
            Some(b'+') => {
                self.position += 1;
                self.unary()
            }
            _ => self.power(),
        };
        self.depth -= 1;
        value.filter(|v| v.is_finite())
    }

    /// 数字或括号。
    fn atom(&mut self) -> Option<f64> {
        if self.peek() == Some(b'(') {
            self.position += 1;
            let value = self.expression()?;
            if self.peek() != Some(b')') {
                return None;
            }
            self.position += 1;
            return Some(value);
        }
        let start = self.position;
        while matches!(self.peek(), Some(b'0'..=b'9' | b'.')) {
            self.position += 1;
        }
        if start == self.position {
            return None;
        }
        std::str::from_utf8(&self.bytes[start..self.position])
            .ok()?
            .parse()
            .ok()
    }
}

/// 整数不带小数点，小数最多十位、去掉末尾的零。
fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if value.abs() >= 1e15 || value.abs() < 1e-10 {
        let scientific = format!("{value:.10e}");
        let (mantissa, exponent) = scientific.split_once('e').unwrap();
        return format!(
            "{}e{exponent}",
            mantissa.trim_end_matches('0').trim_end_matches('.')
        );
    }
    if value.fract() == 0.0 {
        return format!("{value:.0}");
    }
    let text = format!("{value:.10}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic() {
        assert_eq!(evaluate("1+2").as_deref(), Some("3"));
        assert_eq!(evaluate("2x3+4").as_deref(), Some("10"));
        assert_eq!(evaluate("2*(3+4)").as_deref(), Some("14"));
        assert_eq!(evaluate("1/3").as_deref(), Some("0.3333333333"));
        assert_eq!(evaluate("2^10").as_deref(), Some("1024"));
        assert_eq!(evaluate("2^3^2").as_deref(), Some("512"));
        assert_eq!(evaluate("-3+5").as_deref(), Some("2"));
        assert_eq!(evaluate("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(evaluate("1.5x2").as_deref(), Some("3"));
        assert_eq!(evaluate("7%3").as_deref(), Some("1"));
        assert_eq!(evaluate("-2^2").as_deref(), Some("-4"));
        assert_eq!(evaluate("2^-2").as_deref(), Some("0.25"));
        assert_eq!(evaluate("0-0").as_deref(), Some("0"));
        assert_eq!(evaluate("1/100000000000").as_deref(), Some("1e-11"));
        assert_eq!(evaluate("10^20").as_deref(), Some("1e20"));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(evaluate("1+"), None);
        assert_eq!(evaluate("1/0"), None);
        assert_eq!(evaluate("(1+2"), None);
        assert_eq!(evaluate("abc"), None);
        assert_eq!(evaluate(""), None);
        assert_eq!(evaluate("1%0"), None);
        assert_eq!(evaluate("2^9999"), None);
        assert_eq!(evaluate(&"1".repeat(129)), None);
        assert_eq!(
            evaluate(&format!("{}1{}", "(".repeat(33), ")".repeat(33))),
            None
        );
        assert_eq!(evaluate(&format!("{}1", "-".repeat(33))), None);
    }
}
