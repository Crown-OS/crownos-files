//! User and group names from `/etc/passwd` and `/etc/group`, for the
//! Properties dialog.

use std::fs;

const PASSWD: &str = "/etc/passwd";
const GROUP: &str = "/etc/group";

/// The name of user `uid`, or the number when it has none.
pub fn user_name(uid: u32) -> String {
    name_from_file(PASSWD, uid)
}

/// The name of group `gid`, or the number when it has none.
pub fn group_name(gid: u32) -> String {
    name_from_file(GROUP, gid)
}

fn name_from_file(path: &str, id: u32) -> String {
    fs::read_to_string(path)
        .ok()
        .and_then(|table| name_in(&table, id).map(str::to_owned))
        .unwrap_or_else(|| id.to_string())
}

/// The first field of the `name:password:id:…` line with `id`.
fn name_in(table: &str, id: u32) -> Option<&str> {
    table.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let listed = fields.nth(1)?.parse::<u32>().ok()?;
        (listed == id).then_some(name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_names_by_id() {
        let table =
            "root:x:0:0:root:/root:/bin/sh\n# comment\nalice:x:1000:1000::/home/alice:/bin/fish\n";
        assert_eq!(name_in(table, 1000), Some("alice"));
        assert_eq!(name_in(table, 0), Some("root"));
        assert_eq!(name_in(table, 7), None);
        assert_eq!(name_in("users:x:100:alice,bob", 100), Some("users"));
    }
}
