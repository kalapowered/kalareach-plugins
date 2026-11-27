# What an agent qualification record keeps of the person whose home and login a part ran with.
#
# A part run with the person's login publishes only what the driver fills from the build list, the
# run and its own marks, fixed result and failure codes, and counts. Its evidence is projected
# through a schema (evidence_schema): a field the schema does not name is dropped, and a text the
# schema names is kept only where it is a fixed text of the schema's own, a value of the build list's
# entry, or a shape the driver makes (an identifier, one of the part's marks, a path in the run's
# own directory, a time); any other text becomes "<not published>", and a value that is a response
# diagnostic becomes the code it carries. Nothing the agent, its screen, a probe, the person's
# configuration or their files said reaches a record, and neither does the text that described a
# failure: a failure is published as a code and what varies in it, and its reason is the fixed
# text of each code (published_reason). The person's servers are "<server 1>" to "<server n>", and
# what a probe found offered a count. Everything else stays in the part's log and the run's
# evidence directory, which are not published.
#
# A part of the gateway driver writes what it observed under "shows", and beside it the keys it
# searched for, its turn counts, the directories it watched and where the work directory moved.
# Each part has a schema of its own observations (shows_schemas), and a leaf is kept as the
# validator named at its place says, so a change of what a part publishes of a leaf is a change of
# that one line. A watched directory is published only where it is one of watched_directories,
# as the directory, whether it changed and its two entry counts, never a name of an entry; and the
# member, like "shows", is left out where nothing of it is published (without_empty_members).
#
# The driver lists every change in the agent's directories in the person's home, and the person's
# own programs change them too while a part runs: another session's conversation, its lock, a
# cache, a file of their own. A record names a changed file only where it names a conversation a
# part of the record created, or is one of the files the build list names for the agent (guarded,
# shared, recorded, append-only or line files). Every other change is counted, by its category and
# by the listed directory it lies in. Any text of the record that names a path in the person's home
# names no other conversation either, and a path built from the account name keeps "{user}" in its
# place. e2e-agents.sh applies redact_record to each record it assembles. Identifiers are compared
# without regard to case.

# The conversation identifiers a text names: every UUID in it, in either case, in lower case.
def uuid_shape: "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}";
def conversation_ids: [scan(uuid_shape; "i") | ascii_downcase];

# The text as a regular expression that matches it alone.
def literal_pattern: gsub("(?<c>[.+*?()\\[\\]{}^$|\\\\])"; "\\\(.c)");

# The names an agent gives a folder after the path given as input: its slashes joined by dashes, or
# every character that is not a letter or a digit turned into a dash.
def folder_names: [gsub("/"; "-"), gsub("[^A-Za-z0-9]"; "-")] | unique;

# The text with each folder name of the path $path written as $to, where it is the name of a folder
# (after a slash or at the start of the text, and followed by a dash, a slash or the end of it), so
# that a text which only holds the same letters is left as it is.
def hide_folder_named_after($path; $to):
  reduce ($path | folder_names)[] as $name (.;
    gsub("(?<lead>^|/)" + ($name | literal_pattern) + "(?=$|[/-])"; "\(.lead)" + $to));

# The listed directory a "~/..." path lies in: the longest of $roots (the build list's directories
# under "~/", "{date}" standing for a YYYY/MM/DD date and a trailing "/*" for the files directly in
# the directory) that is the path or holds it, or "~/<unlisted>".
def root_of($roots):
  . as $path
  | [ $roots[]
      | . as $root
      | (sub("/\\*$"; "") | split("{date}") | map(literal_pattern)
         | join("[0-9]{4}/[0-9]{2}/[0-9]{2}")) as $pattern
      | select($path | test("^" + $pattern + (if $root | endswith("/*") then "(/[^/]+)?$" else "(/.*)?$" end)))
      | $root ]
  | if length == 0 then "~/<unlisted>" else max_by(length) end;

# Whether a change at the path given as input, which says $text, may be named: every conversation
# $text names is one of $own, and it names one of them or is one of the files the build list names
# for the agent ($named).
def nameable($text; $own; $named):
  . as $path
  | ($text | conversation_ids) as $ids
  | all($ids[]; . as $id | any($own[]; . == $id))
    and (($ids | length) > 0 or any($named[]; . == $path));

# One test's list of changes in the person's home: the entries it may name, and how many others
# lie in each listed directory. An entry of a category that says why a path could not be read
# ($annotated) is never named, since where its path ends cannot be told from the text, and is
# counted under the directory the text before its first ": " lies in.
def split_changes($annotated; $own; $roots; $named):
  reduce .[] as $entry ({named: [], others: {}};
    ($entry | tostring) as $text
    | (if $annotated then $text | split(": ")[0] else $text end) as $path
    | if ($annotated | not) and ($path | nameable($text; $own; $named)) then .named += [$entry]
      else .others[$path | root_of($roots)] += 1 end);

# A test's person_home evidence with every change the record may not name counted instead, under
# "others", by category and listed directory. Files the part created and removed are its own.
def redact_person_home($own; $roots; $named):
  if type != "object" then . else
    . as $home
    | reduce (keys_unsorted[] | select(. != "created_and_removed" and . != "upkeep" and ($home[.] | type) == "array"))
        as $category ($home;
          ($home[$category]
           | split_changes($category == "unread_after" or $category == "left_unsearched";
               $own; $roots; $named))
            as $split
          | .[$category] = $split.named
          | if ($split.others | length) > 0 then .others[$category] = $split.others else . end)
  end;

# A text of several lines as its first line and a note that the rest is in the part's log.
def first_line:
  if test("\n") then split("\n")[0] + " (continued in the part's log)" else . end;

# A text naming a path in the person's home, made fit for a record. A text of several lines that
# names one keeps its first line only, the rest being in the part's log. Then, from the first "~/"
# on, where what follows names any identifier that is not one of $own, all of it becomes the listed
# directory of the path it begins with and "/<not named>": no part of another conversation's file
# name is left, whatever it holds (a time, a space, a quote, "; "), at the cost of the rest of that
# line, such as a host object's identifier after the path, which the part's structured evidence
# keeps. What comes before the first "~/" stays, and so does a text naming no other identifier
# after it: a fixed file of the agent, the ledger, the login keychain.
def redact_text($own; $roots):
  if test("~/") | not then . else
    first_line
    | (capture("^(?<head>.*?)(?<region>~/.*)$") // null) as $parts
    | if $parts == null then .
      elif all($parts.region | conversation_ids[]; . as $id | any($own[]; . == $id)) then .
      else $parts.head
        + (($parts.region | capture("^(?<path>~/[^\\s\"']*)").path | root_of($roots)
            | sub("/\\*$"; "")) + "/<not named>")
      end
  end;

# --- The person's servers ---------------------------------------------------------------------

# A server's placeholder, "<server n>", n being the place of its name among $servers, which lists
# each server the record names once in the order it meets them. A name is only ever a member of
# that list: what a text says is never searched for a name, so no fixed text or build list value is
# changed because a server has the same name, and a name that looks like a placeholder is a name.
def placeholder_of($servers):
  . as $name
  | "<server \(($servers | index([$name])) + 1)>";

# --- What a text may be -----------------------------------------------------------------------

def not_published: "<not published>";

# The shapes the driver makes: [pattern, flags].
def shapes: {
  uuid: ["^" + uuid_shape + "$", "i"],
  mark: ["^kr[0-9a-f]{12}$", ""],
  code_mark: ["^KR[0-9A-F]{12}$", ""],
  mark_start: ["^KR[0-9A-F]{12}-START$", ""],
  mark_end: ["^KR[0-9A-F]{12}-END$", ""],
  mark_head: ["^KR[0-9A-F]{2}$", ""],
  probe_mark: ["^zqkr[0-9a-f]{12}$", ""],
  run_dir: ["^<tmp>/krm-[0-9a-f]{8}$", ""],
  run_png: ["^<tmp>/krm-[0-9a-f]{8}/w/kr[0-9a-f]{12}\\.png$", ""],
  run_command: ["^echo kr[0-9a-f]{6}(-[0-9]+)? >> (<tmp>/krm-[0-9a-f]{8}/w/)?[a-z0-9._-]{1,20}$", ""],
  conversation: ["^((session_)?" + uuid_shape + "|ses_[A-Za-z0-9]{20,40})$", "i"],
  question: ["^What is [0-9]{3} plus [0-9]{3}\\? Reply with only the number\\.$", ""],
  sum: ["^[0-9]{3,4}$", ""],
  sha256: ["^[0-9a-f]{64}$", ""],
  semver: ["^[0-9]+\\.[0-9]+\\.[0-9]+$", ""],
  profile_id: ["^lp-[0-9]+-" + uuid_shape + "$", "i"],
  home_path: ["^~/[A-Za-z0-9._/{}-]+$", ""],
  session_path: ["^(<tmp>/krm-[0-9a-f]{8}/agent/(current/bin|runtime)|/usr/bin|/bin)(:(<tmp>/krm-[0-9a-f]{8}/agent/(current/bin|runtime)|/usr/bin|/bin))*$", ""]
};

# The stop classes a failure can name, each with the words that say it.
def stop_classes: {
  login_not_established: "the agent's login is not established",
  isolation_not_established: "the agent's isolation from the person's own settings and servers is not established",
  guarded_file_changed: "a file of the person's that the part watches changed",
  subagent_started: "the agent started a subagent, whose requests to the model the turn ledger does not count",
  appended_file_changed: "a line of a file the agent only appends to changed or went",
  person_file_unreadable: "a file of the person's could not be read after the part",
  lines_not_removed: "the part's lines could not be removed from a file of the person's",
  configuration_directory_left: "the run's configuration directory is still there after its removal",
  process_outlived_the_run: "something the part started still ran after the run ended everything",
  agent_directories_unread: "the person's agent directories could not be read whole after the part",
  agent_rewrote_files: "the agent rewrote or removed files it had before the part",
  run_data_left: "what the run left in the person's data directory could not be removed",
  workspace_list_changed: "the person's list of workspaces changed other than by gaining the run's folder",
  secret_found: "a string of the login's files was found in what the run wrote, or the search for it was not complete",
  provider_key_found: "the value of a provider key the person's shell holds was found in what the run wrote",
  uncharged_turns: "the agent's record holds prompts the part did not charge, or they could not be counted"
};

# The host's error codes, and the name the host's own code gives each (AMBIGUOUS_ATTACHMENT is
# AmbiguousAttachment), which a part writes where it names the error a call was refused with.
def known_error_codes: ["INVALID_ARGUMENT", "UNSUPPORTED_SCHEMA", "UNSUPPORTED_CAPABILITY", "PERMISSION_DENIED", "PAIRING_EXPIRED", "PAIRING_REJECTED", "PAIRING_AUTH_FAILED", "PAIRING_ATTEMPTS_EXHAUSTED", "RENDEZVOUS_UNAVAILABLE", "RENDEZVOUS_CONFIG_ERROR", "UNKNOWN_SESSION", "AMBIGUOUS_SESSION", "AMBIGUOUS_ATTACHMENT", "TERMINAL_UNAVAILABLE", "TERMINAL_PROBE_FAILED", "INPUT_INCOMPATIBLE", "SESSION_CLOSED", "SESSION_LIMIT", "RESOURCE_UNAVAILABLE", "HOST_NOT_CONFIGURED", "ENVIRONMENT_UNAVAILABLE", "DESKTOP_UNAVAILABLE", "STALE_SESSION", "LEASE_LOST", "GEOMETRY_NOT_OWNER", "DRAFT_CONFLICT", "EDITOR_BUSY", "ID_CONFLICT", "UPSTREAM_UNAVAILABLE", "OUTCOME_UNKNOWN", "RESYNC_REQUIRED", "QUOTA_EXCEEDED", "RATE_LIMITED", "SERVICE_CAPACITY", "CLOCK_UNTRUSTED", "STORAGE_UNAVAILABLE", "SHELL_INTEGRATION_UNSUPPORTED", "ATTACHMENT_INTEGRITY", "REPOSITORY_UNTRUSTED", "PACKAGE_UNAVAILABLE_OFFLINE", "PLUGIN_GRANT_REQUIRED", "PLUGIN_DISABLED", "QUESTION_RESOLVED", "QUESTION_EXPIRED", "NOT_IN_KR_SESSION", "OWNER_CONFIRMATION_REQUIRED", "CAUSAL_LIMIT", "SOURCE_CHANGED"];
def host_name_of_code: ascii_downcase | split("_") | map((.[0:1] | ascii_upcase) + .[1:]) | join("");

# The failure codes a part can carry.
def known_codes: [
  "not_selected", "build_blocked", "no_account", "agent_stopped", "no_ledger", "variable_missing",
  "budget_held", "ledger_unreadable", "login_files_unreadable", "budget_short", "upload_refused",
  "upload_transferred_another_image", "launch_not_detected", "resume_forks", "reply_before_record",
  "not_pinned", "environment_not_clear", "environment_not_read", "key_scan_incomplete", "part_failed",
  "exited_after_outcome", "installed_other_package", "agent_stops", "queued_prompt_not_during_turn"
];

# The texts a part's evidence says of itself that are the driver's own and fixed, by the name a
# schema gives them.
def sets: {
  modes: ["native_terminal", "gateway", "native_bridge"],
  gateway_route: ["the application stand-in", "the pinned build"],
  bypasses: ["not_integrated", "disabled", "absolute_path", "unmanaged_shell", "not_interactive", "backend_unavailable", "session_closing"],
  refusal: ["it was not launched through the integration, so no registration names it and none of its bridges is admitted"],
  identity: ["unproven: no read the host serves names the executable or the release an instance it detected runs; the image the launched process maps is under provenance"],
  admission: ["the agent's conversation held the prompt, and no screen the device was sent from the submission until it disconnected showed the reply mark or the code in upper case"],
  boundary: ["an event or output byte the device was sent between the submission and the agent's record of the prompt showed the reply, the device was sent or asked for a fresh screen in that time, or its reader was not seen to stop once it disconnected"],
  steering_why: ["a prompt entered during a turn waits for it, as the queued step shows"],
  who: ["the paired device, which held the input lease", "the local terminal"],
  receipt: ["input lease"],
  colour: ["red"],
  checks: ["queued", "steered"],
  wrongs: ["a turn started between the prompts", "entered with no turn running", "the agent kept no record of the prompt as queued", "the prompt joined before the turn finished", "the prompt joined the running turn", "the turn finished its work"],
  control_what: ["a second approval with the local terminal holding the lease: it denied and the device's allow was refused", "orders made wrong on purpose, which the queue and steering checks reject", "session B's marker typed into session A", "the same prompt sent again", "a second approval answered by the local terminal alone"],
  control_why_not: ["the controls are the agent's own terminal keys, which only the agent could refuse, so the checks are shown wrong orders instead", "the controls are the agent's own terminal keys, which only the agent could refuse, so the checks are shown wrong orders instead (the guard cases)", "no second execution ran, so there was no second session to type a marker from"],
  origins: ["build", "newer build", "runtime", "run", "shell", "system", "elsewhere"],
  sessions: ["A", "B"],
  parts: ["1", "2a", "2b", "2c", "3", "4", "5a", "5b", "6a", "6b", "7", "8a", "8b", "14.03a", "14.03b"],
  codes: known_codes,
  stop_classes: (stop_classes | keys),
  error_codes: known_error_codes,
  refusal_names: (known_error_codes | map(host_name_of_code)),
  capabilities: ["agent.prompt", "agent.prompt.queue", "agent.steer", "agent.cancel", "agent.approval", "agent.approval.inspect", "agent.approval.respond", "agent.approval.respond/1", "agent.attachment", "agent.binding", "agent.commands", "agent.snapshot", "agent.turn.steer", "agent.turn.cancel", "agent.draft.add_attachment", "terminal.direct", "terminal.input", "terminal.write", "terminal.resize", "terminal.geometry", "terminal.geometry.transfer", "terminal.palette", "terminal.palette.set"],
  causes: ["ended", "not_one_instance", "another_package", "not_native_terminal", "no_refusal", "no_binding", "binding_not_native_terminal", "binding_profile", "no_live_binding"],
  calls: ["upload.begin", "agent.approval.respond", "agent.capabilities", "agent.commands", "agent.draft.add_attachment", "agent.prompt.queue", "agent.prompt.submit", "agent.turn.cancel", "agent.turn.steer"],
  system_programs: ["/bin/ps", "/usr/bin/top", "/usr/bin/sudo", "/usr/bin/su", "/usr/bin/login"],
  categories: ["created_and_left", "appended", "rewritten", "changed_uncompared", "removed_by_something_else", "left_holding_the_part", "left_unsearched", "unread_after"],
  arms: ["pinned build"],
  transfers: ["paired device", "kr-client uploads::send", "paired host's data streams"],
  media_types: ["image/png"],
  capability_states: ["qualified_available", "version_qualified", "missing_installation", "permission_required", "incompatible", "temporarily_unavailable", "not_tested"],
  resume_outcomes: ["refused", "merged", "forked", "ended"],
  moved_work: ["none beside the run"],
  watched_directories: ["~/.codex"]
};

# The regular expression a build list path stands for: "{config}" is a directory of the run's own,
# "{work}" its working directory, "{home}" the person's home, "{user}" the account name or its
# placeholder.
def absent_pattern($user):
  [scan("\\{(?:config|work|home|person|user)\\}|[^{]+|\\{")]
  | map(if . == "{config}" then "<tmp>/krm-[0-9a-f]{8}/[A-Za-z0-9._-]+"
        elif . == "{work}" then "<tmp>/krm-[0-9a-f]{8}/w"
        elif . == "{home}" then "(~|<tmp>/krm-[0-9a-f]{8}/h)"
        elif . == "{person}" then "~"
        elif . == "{user}" then "(\\{user\\}|" + ($user | literal_pattern) + ")"
        else literal_pattern end)
  | "^" + join("") + "$";

# The regular expression a switch of the build list stands for: "{run}" is the run's own directory
# and "{work}" its working directory.
def switch_pattern:
  [scan("\\{(?:run|work)\\}|[^{]+|\\{")]
  | map(if . == "{run}" then "<tmp>/krm-[0-9a-f]{8}"
        elif . == "{work}" then "<tmp>/krm-[0-9a-f]{8}/w"
        else literal_pattern end)
  | "^" + join("") + "$";

# What a part's evidence may say, for the build list entry $entry: the build list's own strings, the
# files of the build's own directory, the person's servers, the conversations the record names as its
# own, and the listed directories.
def context($entry; $user; $servers; $own; $roots; $named):
  ($entry.account // {}) as $account
  | ([($account.isolated // [])[]
      | ((.arguments // []) + (.shows // []) + (.lines // []) + (.lacks // [])
         + [(.accepted // {}) | .starts, .file] + [.shows_within])
      | .[] | strings]) as $raw
  | {
      entry: $entry, account: $account, user: $user, servers: $servers, own: $own, roots: $roots,
      named: $named,
      actions: ($entry.actions // []), grants: ($entry.grants // []),
      lists: {
        arguments: ($account.arguments // []), cleared: ($account.cleared // []),
        decision_calls: ($account.decision_calls // []), call_lines: ($account.call_lines // []),
        hosts: ($account.confinement.hosts // []), unasked_tools: ($account.confinement.unasked_tools // []),
        reported: ($account.confinement.reported // []), residuals: ($account.confinement.residuals // []),
        resume_refused: ($account.resume_refused // [])
      },
      probe_strings: ($raw | unique),
      control_names: ([($account.isolated // [])[] | (.lacks // [])[] | select(. != "{servers}")] | unique),
      absent_patterns: [($account.absent // [])[] | absent_pattern($user)],
      switch_patterns: [($account.switches // [])[] | switch_pattern],
      images: [("^" + ("<tools>/" + ($entry.prefix // "") | literal_pattern) + "/[A-Za-z0-9._@+/-]+$"),
               (if $entry.newer then "^" + ("<tools>/" + $entry.newer.prefix | literal_pattern) + "/[A-Za-z0-9._@+/-]+$" else empty end)],
      resume_words: [($account.resume // [])[] | select(test("^\\{") | not)]
    };

# The switches a part gave the agent: the build list's own, each matching its entry, and after them
# the words that switch off each server, built from the servers' placeholders.
def published_switches($c):
  select(type == "array")
  | ($c.account.switches // []) as $own
  | ($c.account.server_switches.switch // []) as $template
  | ($own | length) as $n
  | .[0:$n] as $head
  | .[$n:] as $tail
  | ($c.part_servers | map(select(type == "string") | . as $name | $template | map(split("{name}") | join($name))) | add // []) as $expected
  | select(($head | length) == $n)
  | [range(0; $n) | . as $j | $head[$j] | if type == "string" and test($c.switch_patterns[$j]) then . else not_published end]
    + (if $tail == $expected
       then ($c.part_servers | map(select(type == "string") | placeholder_of($c.servers) as $p | $template | map(split("{name}") | join($p))) | add // [])
       else ($tail | map(not_published)) end);

# The changes in the person's home that were counted, by category and listed directory.
def published_others($c):
  select(type == "object")
  | with_entries(select(.key | IN(sets.categories[])))
  | map_values(select(type == "object")
      | with_entries(select((.key | IN($c.roots[], "~/<unlisted>")) and (.value | type) == "number")));

# The value where it is valid for the validator $v, transformed where it says, and nothing where it
# is not.
def valid($v; $c):
  . as $x
  | if $v == "bool" then select(type == "boolean")
    elif $v == "number" then select(type == "number")
    elif $v == "count" then (if type == "array" then length elif type == "number" then . else empty end)
    elif $v == "flag" then (if type == "boolean" then . else true end)
    elif $v == "millis" then select(type == "number" or (type == "string" and test("^[0-9]{10,16}$")))
    elif $v == "time" then select(type == "number" or (type == "string"
      and test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]{1,9})?(Z|[+-][0-9]{2}:?[0-9]{2})?$")))
    elif ($v | startswith("in:")) then select(type == "string" and (. as $s | any(sets[$v[3:]][]; . == $s)))
    elif ($v | startswith("build:")) then
      ($c.entry | getpath($v[6:] | split("."))) as $expected | select($expected != null and $x == $expected)
    elif ($v | startswith("member:")) then select(type == "string" and (. as $s | any($c.lists[$v[7:]][]; . == $s)))
    elif ($v == "code_of") then
      select(type == "string")
      | (if IN(sets.error_codes[]) then . else capture("^[a-z_.]+: (?<code>[A-Z][A-Z0-9_]{2,40}):").code end)
      | select(IN(sets.error_codes[]))
    elif $v == "call" then
      select(type == "string" and (IN(sets.calls[])
        or (startswith("plugin.action.invoke ")
            and (ltrimstr("plugin.action.invoke ") as $action | any($c.actions[]; . == $action)))))
    elif $v == "own_path" then
      select(type == "string" and test(shapes.home_path[0]) and (conversation_ids | length) > 0)
    elif $v == "own_id" then
      select(type == "string" and test(shapes.conversation[0]; "i")
             and (ascii_downcase | sub("^session_"; "") as $id | any($c.own[]; . == $id)))
    elif $v == "named_file" then select(type == "string" and (. as $f | any($c.named[]; . == $f)))
    elif $v == "probe_string" then select(type == "string" and (. as $s | any($c.probe_strings[]; . == $s)))
    elif $v == "absent_path" then select(type == "string" and (. as $s | any($c.absent_patterns[]; . as $p | $s | test($p))))
    elif $v == "control_added" then
      select(type == "string" and startswith("nested control/")
             and (ltrimstr("nested control/") as $name | any($c.control_names[]; . == $name)))
    elif $v == "check_result" then select(type == "string") | (if . == "passed" then . else "failed" end)
    elif $v == "build_image" then select(type == "string" and (. as $s | any($c.images[]; . as $p | $s | test($p))))
    elif $v == "resume_word" then
      select(type == "string"
             and ((test(shapes.conversation[0]; "i")
                   and (ascii_downcase | sub("^session_"; "") as $id | any($c.own[]; . == $id)))
                  or (. as $s | any($c.resume_words[]; . == $s))))
    elif $v == "started_word" then
      select(type == "string")
      | if . == "{conversation}" or (. as $s | any(($c.account.resume // [])[]; . == $s)) then .
        elif test(shapes.conversation[0]; shapes.conversation[1]) then "{conversation}"
        else empty end
    elif $v == "code_within" then
      select(type == "string")
      | . as $text
      | [sets.error_codes[] | select(. as $code | $text | test("(^|[^A-Z0-9_])" + $code + "($|[^A-Z0-9_])"))]
      | select(length == 1)
      | .[0]
    elif $v == "capability_state" then
      select(type == "array" and length == 2 and all(.[]; type == "string")
             and (.[0] as $id | any($c.grants[]; ("plugin." + . + "/1") == $id))
             and (.[1] as $state | any(sets.capability_states[]; . == $state)))
    elif $v == "watched_trees" then
      select(type == "array")
      | [.[] | select(type == "object"
                      and (.path | type) == "string"
                      and (.path as $path | any(sets.watched_directories[]; . == $path))
                      and (.changed | type) == "boolean"
                      and (.entries_before | type) == "number"
                      and (.entries_after | type) == "number")
         | {path, changed, entries_before, entries_after}]
      | select(length > 0)
    elif $v == "servers" then select(type == "array") | map(select(type == "string") | placeholder_of($c.servers))
    elif $v == "grant" then select(type == "string" and (. as $s | any($c.grants[]; . == $s)))
    elif $v == "switches" then published_switches($c)
    elif $v == "others" then published_others($c)
    elif (shapes | has($v)) then select(type == "string" and test(shapes[$v][0]; shapes[$v][1]))
    else error("record-redaction: no validator named " + $v) end;

# What a part's failures publish, each as its code and the parameters that are valid for it.
def code_schema: {
  code: "in:codes", refused: "in:error_codes", session: "in:sessions", cause: "in:causes", announced: "number",
  after: "in:parts", variable: "build:account.variable", budget: "build:account.budget",
  left: "number", limit: "number", need: "number", part: "in:parts", class: "in:stop_classes"
};

# The marker that a value is left out of its object or array.
def omitted: {"omitted": true};

# The input as the schema says: a schema that is a string names a validator (a trailing "?" leaves
# a value that is not valid out instead of marking it "<not published>"), one that is an array holds
# the schema of every element, and one that is an object holds the schema of each key it names,
# every other key being dropped. A null stays null.
def project($schema; $c):
  if . == null then null
  elif $schema == "failure_codes" then
    if type == "array" then
      [.[] | select(type == "object" and (.code | type) == "string")
       | select(.code as $code | any(known_codes[]; . == $code))
       | project(code_schema; $c)]
      | reduce .[] as $code ([]; if any(.[]; . == $code) then . else . + [$code] end)
    else not_published end
  elif ($schema | type) == "string" then
    . as $value
    | ($schema | endswith("?")) as $optional
    | ([$value | valid($schema | rtrimstr("?"); $c)]
       | if length == 1 then .[0] elif $optional then omitted else not_published end)
  elif ($schema | type) == "array" then
    if type == "array" then [.[] | project($schema[0]; $c) | select(. != omitted)] else not_published end
  elif type == "object" then
    . as $value
    | reduce ($value | keys_unsorted[]) as $key ({};
        if $schema | has($key) then
          ($value[$key] | project($schema[$key]; $c)) as $out
          | if $out == omitted then . else .[$key] = $out end
        else . end)
  else not_published end;

# --- What a login part's evidence keeps ------------------------------------------------------

def answer_schema: {call: "call", refused: "in:error_codes", error: "flag"};

def detection_schema: {
  instances: [{
    application_instance_id: "uuid", plugin_id: "build:package", profile_id: "profile_id",
    mode: "in:modes", bypass: "in:bypasses", started_at: "millis", ended_at: "millis",
    refusal: "in:refusal"
  }],
  waited_ms: "number"
};

def detected_schema: {detected: "bool", instance: "uuid", identity: "in:identity"};

def order_schema: {
  busy_at_submission: "bool", first_finished: "number", second_prompt: "number",
  second_answered: "number", new_turn: "bool", queued_record: "number", records_queue: "bool",
  before: {busy: "bool", unfinished: "bool"}, after: {busy: "bool", unfinished: "bool"}
};

def session_schema: {
  detection: detection_schema, detected: detected_schema, owners: [{pid: "number"}],
  resumed_with: ["resume_word"], same_conversation: "bool", wrote_into_a_conversation: "bool"
};

def tools_offered_schema: [{
  probe: ["probe_string"], control: {added: "control_added?", server: "bool", rejected: "bool"},
  check: "check_result", tools: "count"
}];

def account_schema: {
  login: "build:account.login", stored: "build:account.stored", home: "build:account.home",
  variable: "build:account.variable", arguments: ["member:arguments"], mirror: "bool",
  variables_absent: ["member:cleared"], turns: "number", budget_spent: "number",
  budget_limit: "number", declined_requests: "count",
  isolation: {
    probes: [{
      command: ["probe_string"], shows: ["probe_string"], lines: ["probe_string"],
      lacks: ["probe_string"], accepted: {starts: "probe_string", file: "probe_string"},
      shows_within: "probe_string"
    }],
    tools_offered: tools_offered_schema, switches: "switches", servers_switched_off: "servers",
    absent_before_start: ["absent_path"], servers: {command: ["probe_string"]}
  }
};

def provenance_schema: {
  session_path: "session_path", sample_interval_ms: "number", samples: "number",
  pids: ["number"],
  executed: [{
    from: "in:origins", pid: "number", device: "number", inode: "number", sha256: "sha256",
    image: "build_image?"
  }],
  launches: [{
    launch: "build:launch", command: "build:command", file: "build_image", sha256: "sha256",
    agent_process: {from: "in:origins", pid: "number", sha256: "sha256", image: "build_image?"}
  }],
  ended_before_read: "count",
  system_programs_of_another_user: [{image: "in:system_programs?", pid: "number"}]
};

def person_files_schema: {
  files: [{
    file: "named_file", changed: "bool", size_changed: "bool", modified_changed: "bool",
    shared: "bool", recorded_only: "bool", names_run: "bool"
  }],
  appended: [{
    file: "named_file", earlier_lines_intact: "bool", appended: "number", others: "number",
    own: [{id: "own_id", time: "time", holds_the_mark: "bool", names_the_run_directory: "bool",
           names_a_conversation_it_created: "bool"}],
    error: "flag"
  }],
  lines_removed: [{file: "named_file", removed: "number", error: "flag"}],
  searched_for: {mark: "mark", run_directory: "run_dir"},
  keychain_item_rewritten: "bool", config_directory_removed: "bool",
  trees: "watched_trees?"
};

# A member of a part's evidence that holds nothing of what the schema keeps is left out, not
# published as an empty object.
def without_empty_members:
  reduce ("person_files", "shows") as $key (.; if .[$key] == {} then del(.[$key]) else . end);

def person_home_schema: {
  created_and_removed: ["own_path?"], created_and_left: ["home_path?"], appended: ["home_path?"],
  rewritten: ["home_path?"], changed_uncompared: ["home_path?"],
  removed_by_something_else: ["home_path?"], left_holding_the_part: ["home_path?"],
  left_unsearched: ["home_path?"], read_whole_after: "bool", unread_after: ["home_path?"],
  upkeep: [{
    path: "member:reported", rewritten: "number", created: "number", bytes: "number",
    modified_ms: "number?", sha256: "sha256?"
  }],
  others: "others"
};

# What a confined agent's part says of its sandbox, its proxy, the data it left and what it showed
# before its first turn: booleans, counts, the profile's digest and the build list's own hosts and tools.
def confinement_schema: {
  profile_sha256: "sha256",
  proxy: {
    hosts: ["member:hosts"], tunnels: "number", refused: "number",
    by_host: [{host: "member:hosts", tunnels: "number", sent: "number", received: "number", open: "number"}]
  },
  unasked_tools: ["member:unasked_tools"],
  residuals: ["member:residuals"],
  settings: {rules: "number", allow_built_in: "number", mode_manual: "bool", loads_more: "bool", unlisted: "number"},
  servers_switched_off: "number",
  left_in_data: {
    trust_records_removed: "number", trust_records_left: "number",
    sessions_bucket_existed: "bool", sessions_bucket_left: "bool", file_history_removed: "number"
  },
  workspaces: {additive: "bool", restored: "bool"},
  secrets: {
    strings: "number", found_in_run: "number", found_in_data: "number", places_in_data: "number",
    intermediate: "number", complete: "bool"
  },
  other_writer_seen: "bool",
  subagent_started: "bool",
  turns: {charged: "number", prompts: "number", steers: "number", others: "number", titles: "number"},
  new_sessions: "number",
  resumed_launches: "number",
  fresh_screen_ms: ["number"],
  zero_turn: [{
    servers_disabled: "number", server_list_complete: "bool", canary_read_outside_refused: "bool",
    canary_read_inside_works: "bool", write_outside_refused: "bool",
    direct_connection_v4_refused: "bool", direct_connection_v6_refused: "bool",
    proxy_refused_a_host_it_was_not_given: "bool", proxy_tunnelled_to_a_host_it_was_given: "bool",
    person_copy_cannot_run: "bool", connections_only_to_the_proxy: "bool", connections_seen: "number",
    device_keys_reach_the_composer: "bool"
  }]
};

# What a gateway part says of the keys of the login that it searched for, and of its turns: the
# entry's own variable name, counts and booleans.
def keys_schema: {
  variable: "build:account.variable", held_by: "count", held_by_not_listed: "number",
  unread_not_listed: "number", complete: "bool", run_gone: "bool"
};

def turns_schema: {
  charged: "count", counted_by_occurrence_of_their_line: "number", in_the_agents_record: "number",
  named_by_a_name_that_is_no_uuid: "number", named_by_uuid: "number", record_files_read: "number",
  forms_of_the_names_that_are_no_uuid: "count"
};

def slash_schema: {typed: "build:account.slash.input", shows: "build:account.slash.shows", seen: "bool"};
def interrupt_schema: {key: "build:account.interrupt.input", shows: "build:account.interrupt.shows", seen: "bool"};

# What every gateway part that names its arm and its login says of them.
def shows_common: {arm: "in:arms", login: "build:account.login", turns_charged: "number"};

# What each gateway part published as its own observations, by part: the leaves of its "shows", each
# with the validator that says what is kept of it. A leaf that is not named here is not published.
def shows_schemas: {
  "1": (shows_common + {
    prompt: {question: "question", answer: "sum"},
    image: {syntax: "build:account.image", answer: "in:colour", code: "code_mark"},
    images_in_the_record: "number", published_is_the_sent: "bool",
    transfer: {
      caller: "in:transfers", driver: "in:transfers", route: "in:transfers",
      published: {
        byte_len: "number", content_digest: "sha256", declared_media_type: "in:media_types",
        presented_as_image: "bool"
      }
    }
  }),
  "2a": {
    arm: "in:arms",
    driven: (shows_common + {
      interrupt: interrupt_schema, slash: slash_schema,
      queued: {answer: "sum", order: order_schema},
      steering: {steered: "bool", order: order_schema},
      control: {what: "in:control_what", why_not: "in:control_why_not", breaks_property: "bool"}
    }),
    agent_reads: [answer_schema],
    surface: {answers: [answer_schema], resources: "number", backend_files: "count", capability_records: "count"},
    capability_states: ["capability_state"],
    detection: detection_schema
  },
  "3": (shows_common + {
    answers_in_the_record: "number", command_ran: "number",
    control: {
      command_ran: "number", device_refused: "bool", breaks_property: "bool",
      what: "in:control_what", check: "flag"
    },
    device_answer: "code_within", device_won: "bool"
  }),
  "4": (shows_common + {
    at_the_drop: {prompts: "number", finished_replies: "number"},
    after_reconnect: {prompts: "number", finished_replies: "number", screen_rows_holding_it: "number", shows: "mark_end"},
    cutoff: {
      fresh_screens: "number", reader_stopped: "bool", resyncs: "number", code_on_a_screen: "bool",
      code_or_reply_mark_in_the_output: "bool", event_not_applied: "bool"
    },
    control: {
      what: "in:control_what", check: "flag", breaks_property: "bool", prompts: "number",
      finished_replies: "number"
    },
    markers: {reply_ends: "mark_end"},
    stale_input: {refused_with: "in:refusal_names"}
  }),
  "6b": {
    first_process_image_unchanged: "bool", fresh_approval_interpreted_by_the_pinned_table: "bool",
    second_instance_has_no_gateway: "bool", second_process_runs_the_newer_image: "bool",
    servers: "number"
  },
  "7": (shows_common + {
    control: {what: "in:control_what", why_not: "in:control_why_not", breaks_property: "bool"},
    files_before_the_second_start: "count",
    resume: {outcome: "in:resume_outcomes", shown: "member:resume_refused"},
    session_a: {detection: detection_schema},
    session_b: {
      same_conversation: "bool", still_running_read_only: "bool", ended_after_the_refusal: "bool",
      started_with: ["started_word"]
    }
  })
};

# The schema of a part's "shows", as the member of the evidence's schema that names it, and none for
# a part that has no observations of its own.
def shows_schema($part): (shows_schemas[$part | tostring] // empty) | {shows: .};

# The evidence of a part run with the person's login.
def evidence_schema: {
  confinement: confinement_schema,
  route: "in:gateway_route", run_started: "bool", turns: turns_schema, keys: keys_schema,
  moved_work_directory: "in:moved_work",
  newer_turns: {after_the_probes: "number", at_the_end: "number", new_since_the_probes: "number"},
  login_held: "bool", stop_agent: "bool", exit: "number", failure_codes: "failure_codes",
  declined_requests: "count", tools_offered: tools_offered_schema,
  key: {variable: "build:account.variable", held_by: "count", complete: "bool", run_gone: "bool"},
  account: account_schema, provenance: provenance_schema, person_files: person_files_schema,
  person_home: person_home_schema, detection: detection_schema, detected: detected_schema,
  device_upload: answer_schema,
  surface: {
    capability_records: [{capability: "in:capabilities", usable: "bool"}],
    answers: [answer_schema], resources: "number", backend_files: "count"
  },
  agent_reads: [answer_schema],
  prompt: {question: "question", answer: "sum"},
  image: {file: "run_png", syntax: "build:account.image", answer: "in:colour", code: "code_mark", answered: "bool"},
  local: {shows: "code_mark", seen: "bool"},
  installed: {plugin_id: "build:package", version: "semver", manifest_digest: "sha256", grant: ["grant"]},
  admission: "in:admission",
  markers: {reply_begins: "mark_start", reply_ends: "mark_end", screen_reply_mark: "build:account.reply_mark", looked_for: "mark_head"},
  cutoff: {reader_stopped: "bool", fresh_screens: "number"},
  reconciled: {attachment: "uuid", epoch: "number", next_sequence: "number", stale_input: "code_of"},
  after_reconnect: {prompts: "number", finished_replies: "number", shows: "mark_end", seen: "bool"},
  slash: slash_schema,
  interrupt: interrupt_schema,
  queued: {answer: "sum", order: order_schema},
  steering: {steered: "bool", why: "in:steering_why", order: order_schema},
  checker_controls: [{check: "in:checks", wrong: "in:wrongs", rejected: "bool"}],
  winner: {who: "in:who", typed: "build:account.approval.allow"},
  loser: {
    who: "in:who", typed: "build:account.approval.deny",
    receipt: {shows: "in:receipt", seen: "bool"}, exit_status: "number"
  },
  command: "run_command",
  executions: {after_the_race: "number", after_reconnecting: "number"},
  decisions: {
    conversation: "conversation", marked_by: "build:account.decision_line",
    answering_calls_marked_by: ["member:decision_calls"], calls_marked_by: ["member:call_lines"],
    after_the_race: "number", after_reconnecting: "number", after_the_control: "number"
  },
  replay: {probe: "probe_mark", typed_again: "bool", screen_checked: "bool", checker_control_rejected: "bool"},
  resources: "number", conversation: "conversation", session_a: session_schema, session_b: session_schema,
  control: {
    what: "in:control_what", breaks_property: "bool", why_not: "in:control_why_not",
    command: "run_command", executions: "number", device_refused: "code_of", check: "flag",
    prompts: "number", finished_replies: "number", isolated: "bool"
  },
  boundary: "in:boundary", code_seen: "bool", reader_stopped: "bool", fresh_screens: "number"
};

# A launch's command line as the agent's own command, which the build list names; the switches it
# was typed with are the account's.
def with_launch_command($c):
  if (.provenance | type) == "object" and (.provenance.launches | type) == "array" then
    .provenance.launches |= map(
      if type == "object" and has("what") then
        (if (.what | type) == "string" and (.what == $c.entry.command or (.what | startswith($c.entry.command + " ")))
         then . + {command: $c.entry.command} else . end)
        | del(.what)
      else . end)
  else . end;

# A control that added one of the person's servers is only said to have: the name is not published,
# wherever the evidence lists what the probes found offered (a part that stopped part way lists it
# at the top).
def with_server_controls:
  def without_names:
    map(if (.control | type) == "object" and .control.server == true then del(.control.added) else . end);
  (if (.account.isolation.tools_offered | type) == "array"
   then .account.isolation.tools_offered |= without_names else . end)
  | (if (.tools_offered | type) == "array" then .tools_offered |= without_names else . end);

# A login part's evidence as its schema says.
def published_evidence($c):
  with_launch_command($c)
  | with_server_controls
  | project(evidence_schema + (shows_schema($c.part) // {}); $c)
  | without_empty_members;

# What a launch's failure to be detected says of its cause.
def cause_text:
  if . == "ended" then "the launched execution had ended"
  elif . == "not_one_instance" then "the host did not announce exactly one live instance for the launched execution"
  elif . == "another_package" then "the one live instance names no package or another package than the installed one"
  elif . == "not_native_terminal" then "the one live instance is not integrated as a native terminal"
  elif . == "no_refusal" then "the one live instance does not say that its bridges are refused"
  elif . == "no_binding" then "a device cannot read the one live instance's binding"
  elif . == "binding_not_native_terminal" then "the instance's binding is not integrated as a native terminal"
  elif . == "binding_profile" then "the instance's binding names no launch profile, or not the one the host announced for the instance"
  elif . == "no_live_binding" then "the host gave no count of live bindings of the package, or counted none"
  else "for a cause the part's log states" end;

# The words of one failure, from its code and the parameters that were valid for it.
def code_text($entry):
  if .code == "not_selected" then "not selected in this run"
  elif .code == "build_blocked" then "the pinned build could not be used on this machine: it is not installed, is not the pinned file, needs a runtime this machine lacks, or its installed code differs"
  elif .code == "no_account" then ($entry.no_account // "the build list names no approved login for this agent (the user)")
  elif .code == "agent_stopped" then "the agent stopped after part \(.after // "?")"
  elif .code == "no_ledger" then "no ledger was named for its turns, so none may start (--turns)"
  elif .code == "variable_missing" then "its login is the variable \(.variable // "?"), which this shell's environment does not hold (the user)"
  elif .code == "login_files_unreadable" then "the login's files cannot be read, so the evidence could not be searched for their strings (the user)"
  elif .code == "budget_held" then "another run holds the \(.budget // "?") budget"
  elif .code == "ledger_unreadable" then "the ledger cannot be read, so what the \(.budget // "?") budget has left is not known"
  elif .code == "budget_short" then "the \(.budget // "?") budget has \(.left // "?") of its \(.limit // "?") turns left, fewer than the \(.need // "?") part \(.part // "?") can start (the ledger)"
  elif .code == "upload_refused" then "a paired device's upload.begin is refused on this host (\(.refused // "?")), so the image is not the device's transfer"
  elif .code == "upload_transferred_another_image" then "a paired device's upload.begin did not transfer the image the part gave: it was accepted with another, or it failed with no code the host refused it with"
  elif .code == "launch_not_detected" then
    "the host did not detect \(if .session then "launch " + .session else "the manual launch" end) as section 12 requires: \(.cause | cause_text) (it announced \(.announced // "?") live instance(s))"
  elif .code == "resume_forks" then "the agent lets no second process write a conversation another process writes, so session B forked the saved conversation under a new identifier: two executions on one conversation's identifier were not shown"
  elif .code == "queued_prompt_not_during_turn" then "the agent was not shown to be running its turn when the second prompt was entered, so the prompt was not shown to wait for a turn that was still running"
  elif .code == "reply_before_record" then "the reply reached the device before the agent recorded the prompt, or whether it had could not be told, so no moment between them was shown"
  elif .code == "not_pinned" then "the session was not shown to run only the pinned build, so the part did not test it"
  elif .code == "environment_not_clear" then "the session's shell exported a variable the build list clears, so the agent was not started or was ended before anything was typed to it"
  elif .code == "environment_not_read" then "the names the session's shell exports could not be read, so that none the build list clears is exported was not established, and the agent was not started or was ended before anything was typed to it"
  elif .code == "key_scan_incomplete" then "the run's directory that held the login's key was not searched whole or is not gone, or something the part started still ran"
  elif .code == "part_failed" then "a check of the part failed; its text is in the part's log"
  elif .code == "exited_after_outcome" then "the part exited non-zero after writing its outcome"
  elif .code == "installed_other_package" then "the host installed a package other than this tree's"
  else empty end;

# A login part's reason: the words of each of its failures, and of the classes the agent stops for,
# in the order the part found them.
def published_reason($entry; $outcome):
  (if type == "array" then [.[] | select(type == "object")] else [] end) as $codes
  | ([$codes[] | select(.code != "agent_stops") | code_text($entry)]) as $texts
  | ([$codes[] | select(.code == "agent_stops") | (stop_classes[.class // ""] // "a reason the part's log states")]) as $stops
  | ($texts + (if ($stops | length) > 0 then ["the agent stops here: " + ($stops | join(", "))] else [] end)) as $all
  | if ($all | length) > 0 then $all | join("; ")
    elif $outcome == "failed" then "a failure the part's log states"
    else "the part did not run; its log states why" end;

# The driver's outcome line of a part that stands as it is, whatever the part's exit status: a
# failure it described, or a not run for a session that was not shown to run only the pinned build
# or whose shell exports a variable the build list clears or could not be read, each by its code.
def driver_line_stands($part):
  select(.part == $part)
  | select(.outcome == "failed"
           or (.outcome == "not_run"
               and ((.evidence.failure_codes // [])
                    | any(.code == "not_pinned" or .code == "environment_not_clear" or .code == "environment_not_read"))));

# The driver's test target a part runs through, and the file that holds its test: the one the
# parts table's row names, else the gateway driver's, or the older driver's for a part that has not
# moved to it.
def part_driver($p): $p.driver // "cases";
def part_source($p):
  $p.source //
  (if part_driver($p) == "gateway" then "kalareach:tests/e2e/agents/tests/gateway.rs"
   else "kalareach:tests/e2e/agents/tests/cases.rs" end);
def part_name($p): "kr-e2e-agents --test " + part_driver($p) + " " + $p.test;

# What a part's entry says of the route it ran on: a part that ran on the stand-in route says so,
# so that nobody reads it as a run of the real agent.
def part_route($p): if $p.route == "stand_in" then {route: "stand_in"} else {} end;

# One part's entry in the record: $p is the parts table's row, $r the driver's last result line of
# the part or null, $step_of and $command_of the run's steps by part.
def part_entry($p; $r; $step_of; $command_of; $entry; $login_parts):
  if $p.test == "" then
    {test: ("part " + $p.part), part: $p.part, source: "kalareach-plugins:fixtures/agents/README.md",
     keyed_by: "case_table", outcome: "not_run",
     reason: ($p.reason + " (" + $p.owner + ")"),
     needs: [$p.needs]}
  elif $r == null then
    {test: part_name($p), part: $p.part, source: part_source($p), keyed_by: "attached_comment",
     outcome: "not_run", reason: "not selected in this run"}
    + part_route($p)
    + (if $entry.account != null and any($login_parts[]; . == $p.part)
       then {evidence: {failure_codes: [{code: "not_selected"}]}} else {} end)
  else
    {test: part_name($p), part: $p.part, source: part_source($p), keyed_by: "attached_comment",
     outcome: $r.outcome}
    + part_route($p)
    + (if $r.reason != null then {reason: $r.reason} else {} end)
    + (if $r.needs != null then {needs: $r.needs} else {} end)
    + (if $step_of[$p.part] != null then
         {runs: [{step: $step_of[$p.part], outcome: $r.outcome}],
          command: $command_of[$p.part]}
       else {} end)
    + {evidence: $r.evidence}
  end;

# The build as the driver reads it: the build list's entry with its paths made absolute under
# $tools, the runtimes $files found, the package's $actions, and the newer build where it is
# installed ($installed) with the commands that say it loads nothing of the person's own.
def driver_build($tools; $root; $files; $actions; $installed):
  {package, actions: $actions, application, version,
   prefix: ($tools + "/" + .prefix), pinned, sha256, command, launch, arguments,
   runtime: [.runtime[] | $files[.]], environment, home, ready, harmless, composer,
   server, transcript, no_account, no_newer,
   account: (if .account.confinement
             then .account | .confinement.profile = ($root + "/" + .confinement.profile)
             else .account end),
   newer: (if .newer == null or $installed != true then null else
     {version: .newer.version, prefix: ($tools + "/" + .newer.prefix), pinned: .newer.pinned,
      sha256: .newer.sha256, ready: .newer.ready, isolated: (.newer.isolated // [])} end)};

# The newer build's isolation commands that a person may adopt from a probe-only run, whose result
# line is the input and whose build list entry is $entry: the pinned entry's commands whose probe the
# run's record shows adoptable, and only where the record allows adoption. The record is evidence for
# a person to read; nothing here judges the answers.
def adopted_isolation($entry):
  (.evidence.shows // {}) as $shows
  | if .outcome == "not_run" and $shows.probe_only == true and $shows.adoption.allowed == true then
      [($entry.account.isolated // [])[]
       | select(.arguments as $arguments
                | any($shows.probes[]?; .arguments == $arguments and .adoptable == true))]
    else [] end;

# A whole record, redacted: $user is the account name the run had, $entry the build list's entry
# for the agent, and $login_parts the parts that run with the login where the entry names one.
def redact_record($user; $entry; $login_parts):
  ($entry.account // {}) as $account
  | ($account.directories // [] | map("~/" + .)) as $roots
  | ([$account.guarded, $account.shared, $account.recorded, $account.append_only,
      $account.line_files] | map(. // []) | add | map("~/" + .)) as $named
  | ([.identifiers[].tests[].evidence | select(type == "object")
      | ((.person_home.created_and_removed // [])[] | strings | conversation_ids[]),
        ((.conversation // empty) | strings | conversation_ids[]),
        ((.decisions.conversation // empty) | strings | conversation_ids[]),
        ((.person_files.appended // [])[].own[]? | select(.names_a_conversation_it_created == true)
         | .id | strings | conversation_ids[])] | unique) as $own
  | ([.identifiers[].tests[].evidence | select(type == "object")
      | (.account.isolation.servers_switched_off // [])[] | strings]
     | reduce .[] as $name ([]; if index([$name]) then . else . + [$name] end)) as $servers
  | context($entry; $user; $servers; $own; $roots; $named) as $context
  | .identifiers[].tests[] |= (
      if (.evidence | type) == "object" and (.evidence | has("person_home"))
      then .evidence.person_home |= redact_person_home($own; $roots; $named) else . end
      # A part the build list's table records as not run has no driver log, so it is not read as a
      # login part that stopped: it stays as the table wrote it.
      | if $entry.account != null and .keyed_by != "case_table"
           and (.part as $part | any($login_parts[]; . == $part)) then
          (if (.evidence | type) == "object" then
             .evidence as $evidence
             | .part as $part
             | .evidence |= published_evidence($context
                 + {part: $part, part_servers: ($evidence.account.isolation.servers_switched_off // [])})
           else . end)
          | . as $test
          | (if $test.outcome == "passed" then del(.reason)
             else .reason = (($test.evidence.failure_codes // []) | published_reason($entry; $test.outcome)) end)
        else . end)
  | walk(if type == "string"
      then redact_text($own; $roots)
        | split("/Library/Managed Preferences/" + $user + "/")
        | join("/Library/Managed Preferences/{user}/")
      else . end);

# A whole record, as one run of a driver is written: $package is the connector package's name,
# $application and $version the build the parts ran against, $started and $finished the run's
# times, $evidence the run's evidence directory, $release the operating system's release,
# $plugins_commit and $core_commit the two repositories' commits, $toolchain the compiler's and
# cargo's versions, $entry the build list's entry for the agent, $steps the run's steps, $parts the
# parts table (one row per part: part, test, reason, owner, needs, driver, route, and the file
# that holds the part's test where it is not the driver's own), $selection the parts that were
# selected, $manifest_version and $manifest_digest the package's release, $generation_identity
# what the run installed packages from, $payload a note on the pinned build, $own_home and
# $own_home_error what the own-home check found or why it failed, $evidence_real, $temporary,
# $temporary_real, $target, $target_real, $shells, $tools, $tools_real and $home the run's
# folders (written relative to themselves in the record), $manifest the package's manifest,
# $pinned_status and $blocked whether the pinned build was installed and why not, $newer_status
# whether the newer build was installed, $user the account name the run had, $login_parts the
# parts that run with the login and $results the driver's result lines, one per outcome. The
# record is redacted by redact_record.
def assemble_record($package; $application; $version; $started; $finished; $evidence; $release;
    $plugins_commit; $core_commit; $toolchain; $entry; $steps; $parts; $selection;
    $manifest_version; $manifest_digest; $generation_identity; $payload; $own_home;
    $own_home_error; $evidence_real; $temporary; $temporary_real; $target; $target_real; $shells;
    $manifest; $pinned_status; $blocked; $newer_status; $tools; $tools_real; $home; $user;
    $login_parts; $results):
  def counts($tests): reduce ("passed", "failed", "ignored", "not_run", "not_built", "known_difference") as $o
    ({}; . + {($o): ([$tests[] | select(.outcome == $o)] | length)});
  # A path in the evidence directory, the temporary directory, the build's target directory, the
  # managed shell's packages, the tools directory or the home directory of whoever ran this is
  # written relative to it, and so is the name an agent gives a folder after such a path (the
  # slashes joined by dashes, or every character that is not a letter or a digit turned into a
  # dash); a system or runtime executable is named as it is, since which one ran is the evidence.
  def local_paths: if type == "string" then
      (split($evidence_real) | join("<evidence>"))
      | (split($temporary_real) | join("<tmp>")) | (split($temporary) | join("<tmp>"))
      | hide_folder_named_after($temporary_real; "<tmp>") | hide_folder_named_after($temporary; "<tmp>")
      | (split($target_real) | join("<target>")) | (split($target) | join("<target>"))
      | (if $shells == "" then . else split($shells) | join("<shells>") end)
      | (split($tools_real) | join("<tools>")) | (split($tools) | join("<tools>"))
      | (if $home == "" then . elif . == $home then "~"
         else split($home + "/") | join("~/")
           | gsub(($home | literal_pattern) + "(?![A-Za-z0-9._-])"; "~")
           | hide_folder_named_after($home; "~") end)
    else . end;
  def verdict($tests): if any($tests[]; .outcome == "failed") then "failed"
    elif any($tests[]; .outcome == "passed") then "passed" else "not_run" end;
  def declared: ($manifest.attachments // null) as $a |
    if $a == null then null
    elif $a.insertion == "upstream_upload" then "typed_submission"
    elif $a.insertion == "native_composer" then "verified_composer_insertion"
    else $a.insertion end;
  ($steps | map(select(.group == "agents")) | map({key: (.what | ltrimstr("part ")), value: .number}) | from_entries) as $step_of |
  ($steps | map(select(.group == "agents")) | map({key: (.what | ltrimstr("part ")), value: .command}) | from_entries) as $command_of |
  [ $parts[] | . as $p |
    part_entry($p; ([$results[] | select(.part == $p.part)] | last); $step_of; $command_of;
               $entry; $login_parts)
  ] as $tests |
  ($tests | map(select(.part | startswith("14.03") | not))) as $case_tests |
  ($tests | map(select(.part | startswith("14.03")))) as $attachment_tests |
  {
    schema: "kalareach.conformance/1",
    extension: "kalareach.agent-qualification/1",
    repository: "kalareach-plugins",
    run: {
      started: $started, finished: $finished, platform: "macos",
      system: {os: "macos", release: $release, arch: "aarch64", target: "aarch64-apple-darwin"},
      commit: $plugins_commit,
      host: {repository: "kalareach", commit: $core_commit},
      toolchain: $toolchain,
      terminal_profile: {profile: "kr-vt/1", term: "ghostty"},
      packages: [{name: $package, version: $manifest_version, manifest_digest: $manifest_digest,
                  generation: $generation_identity}],
      applications: ([{id: $application, version: $version, status: $pinned_status,
                       url: $entry.source, sha256: $entry.sha256, build: "darwin-arm64",
                       reason: (if $blocked == "" then "the build the table is pinned to" + $payload else $blocked end)}]
        + (if $entry.newer == null then [] else
            [{id: $application, version: $entry.newer.version,
              status: (if $newer_status == true then "installed" else "not_installed" end),
              url: $entry.newer.source, sha256: $entry.newer.sha256, build: "darwin-arm64",
              reason: "the newer build the upgrade part moves to"}] end)),
      selection: $selection,
      all_terminals: false,
      evidence_directory: $evidence,
      own_home: $own_home
    },
    steps: $steps,
    identifiers: {
      "KR-REQ-12.32": {family: "requirement", verdict: verdict($case_tests),
                       counts: counts($case_tests), tests: $case_tests},
      "KR-REQ-14.03": {family: "requirement", verdict: verdict($attachment_tests),
                       counts: counts($attachment_tests), tests: $attachment_tests}
    },
    attachment_paths: (
      [$manifest.actions[] | select(.effect == "upstream.prompt" or .effect == "upstream.attachment") |
        {operation: .id, declared: declared, source: "plugin.json", package_digest: $manifest_digest}]
      + [{operation: "terminal", declared: null, source: "plugin.json", package_digest: $manifest_digest}]),
    failures_outside_identifiers: (if $own_home_error == "" then [] else
      [{test: "kr-e2e-agents --test cases a_session_started_with_a_persons_own_home_keeps_their_login_keychain_as_its_default",
        source: "kalareach:tests/e2e/agents/tests/cases.rs", step: 1, reason: $own_home_error}] end),
    known_differences: [],
    problems: [],
    warnings: [],
    summary: {
      identifiers: 2,
      passed: ([verdict($case_tests), verdict($attachment_tests)] | map(select(. == "passed")) | length),
      failed: ([verdict($case_tests), verdict($attachment_tests)] | map(select(. == "failed")) | length),
      known_difference: 0,
      not_run: ([verdict($case_tests), verdict($attachment_tests)] | map(select(. == "not_run")) | length),
      known_differences: 0,
      tests: counts($tests),
      failed_steps: [$steps[] | select(.exit != 0) | .number]
    }
  } | walk(local_paths) | .run.evidence_directory = $evidence
    # What the record may not name of the person: record-redaction.jq.
    | redact_record($user; $entry + {actions: [$manifest.actions[].id], grants: [$manifest.capabilities[].capability]}; $login_parts);
