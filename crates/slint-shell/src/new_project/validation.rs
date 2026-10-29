//! New Project form validation — UX-only early feedback.  The kernel
//! re-validates everything at generation time; this is what gates the
//! Create button and explains why it is disabled.

/// One template input with its live user value.
#[derive(Debug, Clone)]
pub struct VarValue {
    pub key: String,
    pub kind: String,
    pub value: String,
    pub description: String,
    pub required: bool,
}

pub fn validate(name: &str, dest: &str, vars: &[VarValue]) -> (bool, String) {
    if !kernel::templates::generate::valid_project_name(name) {
        return (
            false,
            "Enter a project name (letters, digits, '-', '_')".into(),
        );
    }
    if dest.trim().is_empty() {
        return (false, "Choose a destination folder".into());
    }
    if !std::path::Path::new(dest).is_absolute() {
        return (false, "Destination must be an absolute path".into());
    }
    for var in vars {
        if var.required && var.value.trim().is_empty() {
            return (false, format!("{} is required", var.key.replace('_', " ")));
        }
        if var.kind == "boolean"
            && !var.value.is_empty()
            && var.value != "true"
            && var.value != "false"
        {
            return (
                false,
                format!("{} must be true or false", var.key.replace('_', " ")),
            );
        }
    }
    (true, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rejects_bad_names() {
        assert!(!validate("my project", "/tmp/x", &[]).0);
        assert!(!validate("", "/tmp/x", &[]).0);
    }

    #[test]
    fn validation_requires_absolute_destination() {
        assert!(!validate("proj", "relative/dir", &[]).0);
        assert!(validate("proj", "/tmp/x", &[]).0);
    }

    #[test]
    fn validation_requires_filled_required_vars() {
        let vars = vec![VarValue {
            key: "license".into(),
            kind: "text".into(),
            value: String::new(),
            description: String::new(),
            required: true,
        }];
        let (ok, reason) = validate("proj", "/tmp/x", &vars);
        assert!(!ok);
        assert!(reason.contains("license"));
        let vars = vec![VarValue {
            value: "MIT".into(),
            ..vars[0].clone()
        }];
        assert!(validate("proj", "/tmp/x", &vars).0);
    }

    #[test]
    fn validation_accepts_only_boolean_literals() {
        let vars = vec![VarValue {
            key: "tokio".into(),
            kind: "boolean".into(),
            value: "yes".into(),
            description: String::new(),
            required: false,
        }];
        assert!(!validate("proj", "/tmp/x", &vars).0);
    }
}
