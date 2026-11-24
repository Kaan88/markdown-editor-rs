#!/bin/bash
# revert2.sh: each fix put back in turn; its test must fail. Restores each file from a saved copy.
cd /c/Users/kaank/Desktop/md-wt/a4-lib || exit 1
S=/c/Users/kaank/AppData/Local/Temp/claude/ddd3b02074f601ee/e3635bc8-7486-4b64-a984-53cb1ce200e6/scratchpad
out=$S/revert2.log
: > "$out"
mkdir -p "$S/keep"
check() { # name pkg file perl-expr test
  local name="$1" pkg="$2" file="$3" expr="$4" test="$5"
  cp "$file" "$S/keep/now"
  perl -0pi -e "$expr" "$file"
  if cmp -s "$file" "$S/keep/now"; then echo "$name: REVERT DID NOT APPLY" >> "$out"; return; fi
  local lib=""
  [ "$pkg" = mdedit-core ] && lib="--lib"
  local res
  res=$(cargo test --release -p "$pkg" $lib --config 'build.rustc-wrapper=""' -j 4 -- "$test" 2>&1 | grep -E "^test result|^error\[|^error:" | head -2 | tr '\n' ' ')
  echo "$name [$test]: $res" >> "$out"
  cp "$S/keep/now" "$file"
}
P=bins/mdedit/src/library_pane.rs
D=bins/mdedit/src/app/library_drag.rs
I=crates/mdedit-core/src/import.rs
G=bins/mdedit/src/app/importing.rs
U=bins/mdedit/src/import_ui.rs
R=bins/mdedit/src/import_draw.rs
check C1-scroll mdedit "$D" 's/let over = x >= area\.x && x < area\.right\(\);/let over = true;/' a_note_let_go_over_the_editor_at_a_rows_height_is_not_moved
check C2-keys mdedit "$D" 's/(self\.redraw\(\);\r?\n        \}\r?\n)        true(\r?\n    \})/$1        name == Some(&KeyName::Escape)$2/' a_key_while_a_note_is_held_reaches_nothing
check C4-plan mdedit "$G" 's/\.is_some_and\(\|home\| !home_holds\(home, dir\)\);/.is_some_and(|_| false);/' a_group_of_that_name_whose_folder_is_another
check C4-plan-core mdedit-core "$I" 's/\(!group_new && at\.home_apart\)/(false \&\& at.home_apart)/' a_group_that_keeps_its_notes_in_another_folder
check C4-write mdedit "$G" 's/&& let Some\(home\) = old\.home\.as_deref\(\)/\&\& let Some(home) = None::<\&Path>/' a_group_whose_folder_changed
check C4-line mdedit "$U" 's/\} else if plan\.home\.as_os_str\(\)\.is_empty\(\) \{/} else if false {/' a_group_with_no_folder_of_its_own
check C5-core mdedit-core "$I" 's/if self\.is_library\(&name, &path\) \{/if false \&\& self.is_library(\&name, \&path) {/' the_library_folder_inside_the_folder_walked
check C5-app mdedit "$I" 's/if self\.is_library\(&name, &path\) \{/if false \&\& self.is_library(\&name, \&path) {/' the_folder_that_holds_the_library
check C6-core mdedit-core "$I" 's/if !self\.links_back\(&path, &title\) \{/if false \&\& !self.links_back(\&path, \&title) {/' a_file_whose_line_would_not_read_back
check C6-app mdedit "$I" 's/if !self\.links_back\(&path, &title\) \{/if false \&\& !self.links_back(\&path, \&title) {/' a_file_whose_name_the_library_cannot_hold
check C7-lines mdedit-core "$I" 's/for line in text_lines\(head\) \{/for line in head.lines() {/' a_title_is_one_line
check C7-clean mdedit-core "$I" 's/heading_text\(trimmed\)\.and_then\(\|t\| clean_title\(&t\)\)/heading_text(trimmed).filter(|t| !t.is_empty())/' a_title_is_one_line
check C7-preview mdedit-core "$I" 's/let mut lines: Vec<String> = text_lines\(head\)/let mut lines: Vec<String> = head.lines()/' a_title_is_one_line
check C8-entries mdedit-core "$I" 's/if out\.read > self\.limits\.read \{/if false {/' a_walk_stops_at_its_bounds
check C8-plan mdedit-core "$I" 's/\.map\(stopped_reason\)/.and(None::<String>)/' a_walk_stops_at_its_bounds
check C12h-depth mdedit-core "$I" 's/if depth > self\.limits\.depth \{/if false {/' a_walk_stops_at_its_bounds
check C8-stop mdedit "$G" 's/job\.cancel\(\);/self.import_job = Some(job); return;/' the_command_stops_a_folder_being_read
check C8-hold mdedit "$G" 's/if self\.import_held\(\) \{/if false \&\& self.import_held() {/' a_walk_that_ends_while_another_card
check C10.3-dryrun mdedit "$G" 's/if !runs::check_links_added\(&old, &library::parse\(&new, &library\), &links, &library\) \{/if false {/' a_plan_whose_links_would_not_read_back
check C10.4-junction mdedit-core "$I" 's/if \(kind\.is_symlink\(\) && meta\.is_dir\(\)\)/if (false \&\& meta.is_dir())/' a_junction_to_a_folder_is_not_followed
check C12f-case mdedit-core "$I" 's/\.take_while\(\|\(a, b\)\| a\.eq_ignore_ascii_case\(b\)\)/.take_while(|(a, b)| a == b)/' sections_that_differ_only_in_case
check C12g-dots mdedit-core "$I" 's/tree::same_note\(p, &f\.path\)/same_path(p, \&f.path)/' a_file_the_group_lists_through_dot_dot
check footer mdedit "$R" 's/let said = disabled\.clone\(\)\.or_else\(\|\| self\.refusal\.clone\(\)\);/let said = disabled.clone();/' a_refused_add_is_said_in_the_footer
echo DONE >> "$out"
git status --short >> "$out"
