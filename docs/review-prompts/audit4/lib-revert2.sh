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
N=bins/mdedit/src/app/note.rs
check C12e-button mdedit "$N" 's/(if let Some\(why\) = self\.note_blocked\(&root, &card\.target\) \{\r?\n\s+return self\.)note_write_failed/$1note_write_refused/' a_refusal_at_the_write_that_can_pass_leaves_the_button_on
check sorting-help mdedit "$R" 's/SortStage::Filing \{ \.\. \} => (\{\r?\n\s+"The files are being sent)/SortStage::Asking => $1/' the_schemes_preview_says_what_can_be_done
check failed-demo mdedit "$U" 's/it held no JSON object\."/it held no JSON object. Nothing was written."/' the_failed_sorts_demo_says_nothing_was_written_once
echo DONE >> "$out"
git status --short >> "$out"
