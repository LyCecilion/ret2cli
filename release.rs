#[must_use]
pub fn codename_for_version(package_version: &str) -> Option<&'static str> {
    let mut components = package_version.split('.');
    match (components.next(), components.next()) {
        (Some("1"), Some("0")) => Some("LORELEI"),
        (Some("1"), Some("1")) => Some("Bloom in Two"),
        (Some("2"), Some("0")) => Some("TearJerker"),
        _ => None,
    }
}

#[must_use]
pub fn github_build_metadata(
    run_number: Option<&str>,
    run_attempt: Option<&str>,
    sha: Option<&str>,
) -> Option<String> {
    let run_number = run_number?;
    let run_attempt = run_attempt?;
    let sha = sha?;
    if !is_decimal(run_number) || !is_decimal(run_attempt) || sha.len() < 7 {
        return None;
    }

    let short_sha: String = sha.chars().take(7).collect();
    if !short_sha.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }

    Some(format!("build.{run_number}.{run_attempt}.g{short_sha}"))
}

fn is_decimal(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{codename_for_version, github_build_metadata};

    #[test]
    fn lorelei_covers_the_entire_v1_0_release_line() {
        assert_eq!(codename_for_version("1.0.0"), Some("LORELEI"));
        assert_eq!(codename_for_version("1.0.42"), Some("LORELEI"));
        assert_eq!(codename_for_version("1.0.1-beta.1"), Some("LORELEI"));
    }

    #[test]
    fn bloom_in_two_covers_the_entire_v1_1_release_line() {
        assert_eq!(codename_for_version("1.1.0"), Some("Bloom in Two"));
        assert_eq!(codename_for_version("1.1.42"), Some("Bloom in Two"));
        assert_eq!(codename_for_version("1.1.1-beta.1"), Some("Bloom in Two"));
    }

    #[test]
    fn tear_jerker_covers_the_entire_v2_0_release_line() {
        assert_eq!(codename_for_version("2.0.0"), Some("TearJerker"));
        assert_eq!(codename_for_version("2.0.42"), Some("TearJerker"));
        assert_eq!(codename_for_version("2.0.1-beta.1"), Some("TearJerker"));
    }

    #[test]
    fn a_new_release_line_requires_an_explicit_codename() {
        assert_eq!(codename_for_version("1.2.0"), None);
    }

    #[test]
    fn github_builds_receive_valid_semver_metadata() {
        assert_eq!(
            github_build_metadata(Some("42"), Some("2"), Some("91b1cc1abcdef")),
            Some("build.42.2.g91b1cc1".to_owned())
        );
    }

    #[test]
    fn incomplete_or_invalid_github_metadata_is_ignored() {
        assert_eq!(github_build_metadata(Some("42"), None, Some("91b1cc1abcdef")), None);
        assert_eq!(github_build_metadata(Some("run-42"), Some("2"), Some("91b1cc1abcdef")), None);
        assert_eq!(github_build_metadata(Some("42"), Some("2"), Some("not-a-sha")), None);
    }
}
