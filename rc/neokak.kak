# neokak runtime glue. Sourced by the neokak frontend after the user's kakrc.
#
# The frontend sets $NEOKAK_BIN and $NEOKAK_SOCK in the session environment;
# `neokak msg ...` forwards its arguments to the frontend over that socket.

# Frontend messaging
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾

define-command -hidden neokak-msg -params 1.. %{
    nop %sh{ "$NEOKAK_BIN" msg "$@" >/dev/null 2>&1 </dev/null }
}

define-command -hidden neokak-sync-current %{
    neokak-msg current %val{client} %val{bufname} %val{buffile}
}

define-command -hidden neokak-sync-buflist %{
    neokak-msg buflist %val{buflist}
}

hook global WinDisplay .* %{
    neokak-sync-current
    neokak-sync-buflist
}
hook global BufCreate .* neokak-sync-buflist
hook global BufClose .* %{ neokak-msg bufclose %val{hook_param} }
hook global BufWritePost .* %{ neokak-msg written %val{hook_param} }
# When the session's first client quits while others remain, Kakoune forks
# the server into the background and it keeps that client's stdout open, so
# the frontend can't rely on EOF to notice the client left.
hook global ClientClose .* %{
    # Save the session first: neokak may exit as soon as it hears the last
    # client is gone.
    try neokak-session-sync
    neokak-msg clientclose %val{hook_param}
}

# Statusline data
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾
# The frontend draws a lualine-style statusline itself. It reads its fields
# from the modeline, separated by U+241E so they survive Kakoune's markup.
# The last field is one character per face, so the frontend can read the
# colorscheme's colors (for the "kakoune" theme).

declare-option -hidden str neokak_mode normal
hook global ModeChange '(?:push|pop):.*:(.*)' %{
    try %{ set-option window neokak_mode %val{hook_param_capture_1} }
}

# kak-lsp: keep the user's setup if their kakrc already loaded it.
# NEOKAK_NO_LSP=1 skips loading it here (the test suite uses this).
try %{ nop %opt{lsp_servers} } catch %{
    evaluate-commands %sh{
        [ -z "$NEOKAK_NO_LSP" ] && command -v kak-lsp >/dev/null 2>&1 && kak-lsp --kakoune -s "$kak_session"
    }
    try lsp-enable
}
# Project environments
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾
# When you start neokak in a project or switch to one, neokak loads the
# project's environment (what a shell hook like direnv's would activate) with
# the first provider whose marker file is in the project directory or above.
# A provider command runs in the project directory and prints the
# environment NUL-separated (`env -0`), or nothing when it doesn't apply.
# Add your own, e.g. 'mise.toml:mise exec -- env -0' or
# 'flake.nix:nix develop --command env -0'; clear the option to turn it off.
declare-option -docstring "<marker>:<command> pairs that load a project's environment" \
    str-list neokak_env_providers \
    '.envrc:if command -v direnv >/dev/null; then direnv exec . env -0; fi' \
    'devenv.nix:if command -v devenv >/dev/null; then devenv hook-should-activate >/dev/null && devenv shell -- env -0; fi'
define-command -hidden neokak-sync-env-providers %{
    neokak-msg env-providers %opt{neokak_env_providers}
}
hook global GlobalSetOption 'neokak_env_providers=.*' neokak-sync-env-providers

# neokak then calls this with a shell script that switches a shell to the
# loaded environment. Kakoune's own environment can't change, so kak-lsp is
# restarted through the script; source it in your own %sh{} blocks with
# `. "$kak_opt_neokak_env"` where you need the project's tools.
declare-option -docstring "shell script that switches to the project's devenv/direnv environment" \
    str neokak_env
declare-option -hidden str neokak_lsp_cmd
define-command -hidden neokak-project-env -params 1 %{
    set-option global neokak_env %arg{1}
    try %{
        nop %opt{lsp_cmd}
        evaluate-commands %sh{
            [ -n "$kak_opt_neokak_lsp_cmd" ] || echo 'set-option global neokak_lsp_cmd %opt{lsp_cmd}'
        }
        evaluate-commands %sh{
            cmd=". '$(printf %s "$1" | sed "s/'/'\\\\''/g")' && $kak_opt_neokak_lsp_cmd"
            printf "set-option global lsp_cmd '%s'\n" "$(printf %s "$cmd" | sed "s/'/''/g")"
        }
        lsp-if-running lsp-restart
    }
}

try %{ nop %opt{lsp_diagnostic_error_count} } catch %{
    declare-option -hidden int lsp_diagnostic_error_count 0
    declare-option -hidden int lsp_diagnostic_warning_count 0
}

# LSP result lists in pickers
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾
# kak-lsp shows symbols, references, diagnostics and other lists through its
# hidden lsp-show-goto-buffer command; send them to a neokak picker instead.

declare-option -docstring 'show kak-lsp result lists (symbols, references, diagnostics) in pickers' \
    bool neokak_lsp_pickers true
declare-option -hidden str neokak_runtime %sh{ dirname "$NEOKAK_SOCK" }
# Picker title for the next result list (kak-lsp sends references,
# definitions and implementations all as *goto*).
declare-option -hidden str neokak_lsp_title

define-command -hidden neokak-lsp -params 2 %{
    set-option global neokak_lsp_title %arg{1}
    try %{ evaluate-commands %arg{2} } catch %{
        set-option global neokak_lsp_title ''
        fail %val{error}
    }
}

define-command -override -hidden lsp-show-goto-buffer -params 4..5 %{
    evaluate-commands %sh{
        if [ "$kak_opt_neokak_lsp_pickers" = true ]; then
            echo 'neokak-lsp-picker %arg{@}'
        else
            echo 'neokak-lsp-goto-buffer %arg{@}'
        fi
    }
}

define-command -hidden neokak-lsp-picker -params 4..5 %{
    # Through a file: results can exceed the size limit of one argument.
    echo -to-file "%opt{neokak_runtime}/lsp-results" %arg{4}
    neokak-msg lsp-results %arg{1} %arg{2} %arg{3} "%opt{neokak_runtime}/lsp-results" %opt{neokak_lsp_title}
    set-option global neokak_lsp_title ''
}

# kak-lsp's own behavior, for neokak_lsp_pickers=false.
define-command -hidden neokak-lsp-goto-buffer -params 4..5 %{
    evaluate-commands -save-regs '"' -try-client %opt[toolsclient] %{
        edit! -scratch %arg{1}
        set-option buffer filetype %arg{2}
        set-option buffer jump_current_line 0
        set-option buffer lsp_project_root "%arg{3}/"
        set-register '"' %arg{4}
        execute-keys Rgg
    }
    try %{
        evaluate-commands -try-client %opt[toolsclient] %{
            set-option buffer jump_current_line %arg{5}
            execute-keys %arg{5} g
        }
    }
}

# Hide LSP inlay hints while in insert mode. Kakoune doesn't shift empty
# ranges (which inlay hints are) when lines are inserted above them, so they
# show up on the wrong line until kak-lsp's next refresh.
declare-option -docstring 'hide LSP inlay hints in insert mode' bool neokak_hide_hints_in_insert true
declare-option -hidden str-list neokak_hidden_hints

hook global ModeChange 'push:[^:]*:insert' %{
    set-option window neokak_hidden_hints
    evaluate-commands %sh{
        [ "$kak_opt_neokak_hide_hints_in_insert" = true ] || exit
        for scope in window buffer global; do
            printf 'try %%{ remove-highlighter %s/lsp_inlay_hints; set-option -add window neokak_hidden_hints %s }\n' "$scope" "$scope"
        done
    }
}

hook global ModeChange 'pop:insert:.*' %{
    evaluate-commands %sh{
        eval set -- "$kak_quoted_opt_neokak_hidden_hints"
        [ $# -eq 0 ] && exit
        # Drop hints computed before the edits so they don't flash in the
        # wrong place; kak-lsp's NormalIdle hook fetches fresh ones.
        if [ "${kak_opt_lsp_inlay_hints%% *}" != "$kak_timestamp" ]; then
            echo 'set-option buffer lsp_inlay_hints %val{timestamp}'
        fi
        for scope; do
            printf 'try %%{ add-highlighter %s/lsp_inlay_hints replace-ranges lsp_inlay_hints }\n' "$scope"
        done
        echo 'set-option window neokak_hidden_hints'
    }
}

set-option global modelinefmt '%opt{neokak_mode}␞%val{bufname}␞%val{modified}␞%val{cursor_line}␞%val{cursor_char_column}␞%val{buf_line_count}␞%opt{filetype}␞{{mode_info}}␞%opt{lsp_diagnostic_error_count}␞%opt{lsp_diagnostic_warning_count}␞{Default}D{StatusLine}S{MenuBackground}M{MenuForeground}F{LineNumbers}L{BufferPadding}P{comment}c{function}f{keyword}k{string}s{type}t{value}v{module}m{Error}E'

# Interface theme
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾
# Colors of everything neokak draws itself. Text in the editor keeps the
# Kakoune colorscheme.

declare-option -docstring %{
    neokak interface theme: gruvbox, gruvbox-light, tokyonight, catppuccin, nord, onedark,
    or kakoune to follow the Kakoune colorscheme
} str neokak_theme gruvbox
declare-option -docstring 'draw popups on the terminal background instead of the theme color' bool neokak_transparent true
declare-option -docstring %{
    theme color overrides as name=color, e.g. blue=#7aa2f7 bg_sel=rgb:3b4261
    names: bg_float bg_bar bg_alt bg_sel fg fg_dim border red green yellow blue purple aqua orange
} str-list neokak_theme_colors

define-command neokak-theme -params 1 -docstring %{
    neokak-theme <name>: switch the interface theme
} -shell-script-candidates %{
    printf '%s\n' gruvbox gruvbox-light tokyonight catppuccin nord onedark kakoune
} %{
    set-option global neokak_theme %arg{1}
}

define-command -hidden neokak-sync-theme %{
    neokak-msg theme %opt{neokak_theme} %opt{neokak_transparent} %opt{neokak_theme_colors}
}
hook global GlobalSetOption 'neokak_(theme|transparent|theme_colors)=.*' neokak-sync-theme

# Git
# ‾‾‾
# Change markers in the gutter (live, using Kakoune's git tool), hunk
# navigation, inline blame and a git TUI in a floating terminal.

declare-option -docstring 'show git change markers in the gutter' bool neokak_git_signs true
declare-option -docstring 'show who last changed the cursor line, after the line' bool neokak_git_blame_line false
declare-option -docstring 'program <space>gg runs in a floating terminal' str neokak_git_tool lazygit
declare-option -hidden bool neokak_git_tracked false
declare-option -hidden int neokak_git_timestamp -1

try %{
    set-option global git_diff_add_char '▎'
    set-option global git_diff_mod_char '▎'
    set-option global git_diff_del_char '▁'
    set-option global git_diff_top_char '▔'
}

define-command -hidden neokak-git-check %{
    evaluate-commands %sh{
        [ "$kak_opt_neokak_git_signs" = true ] || exit
        cd "${kak_buffile%/*}" 2>/dev/null &&
            git ls-files --error-unmatch -- "$kak_buffile" >/dev/null 2>&1 &&
            echo 'set-option buffer neokak_git_tracked true'
    }
}
define-command -hidden neokak-git-show %{
    evaluate-commands %sh{
        [ "$kak_opt_neokak_git_tracked" = true ] && [ "$kak_opt_neokak_git_signs" = true ] &&
            echo 'try %{ git show-diff }'
    }
}
hook global BufOpenFile .* neokak-git-check
hook global WinDisplay .* neokak-git-show

# Refresh markers after edits (git update-diff diffs the unsaved buffer).
define-command -hidden neokak-git-update %{
    evaluate-commands %sh{
        [ "$kak_opt_neokak_git_tracked" = true ] && [ "$kak_opt_neokak_git_signs" = true ] &&
            [ "$kak_timestamp" != "$kak_opt_neokak_git_timestamp" ] &&
            echo 'try %{ git update-diff }; set-option buffer neokak_git_timestamp %val{timestamp}'
    }
}
hook global BufSetOption neokak_git_tracked=true %{
    hook -group neokak-git buffer NormalIdle .* neokak-git-update
    hook -group neokak-git buffer InsertIdle .* neokak-git-update
    hook -group neokak-git buffer BufWritePost .* neokak-git-update
    hook -group neokak-git buffer BufReload .* neokak-git-update
}

hook global GlobalSetOption neokak_git_signs=false %{
    evaluate-commands -buffer * %{ try %{ remove-highlighter window/git-diff } }
}

define-command neokak-git-blame-toggle -docstring 'toggle inline blame for the cursor line' %{
    evaluate-commands %sh{
        [ "$kak_opt_neokak_git_blame_line" = true ] && v=false || v=true
        echo "set-option global neokak_git_blame_line $v"
    }
}
hook global GlobalSetOption neokak_git_blame_line=true %{
    remove-hooks global neokak-blame
    hook -group neokak-blame global NormalIdle .* %{
        neokak-msg blame %val{client} %val{buffile} %val{cursor_line} %val{modified}
    }
}
hook global GlobalSetOption neokak_git_blame_line=false %{
    remove-hooks global neokak-blame
    neokak-msg blame-off
}

define-command neokak-float -params 1.. -docstring %{
    neokak-float <program> [<args>...]: run a program in a floating terminal until it exits
} %{
    neokak-msg float %arg{@}
}

# Tree-sitter
# ‾‾‾‾‾‾‾‾‾‾‾
# kak-tree-sitter (when installed) provides syntax highlighting and
# syntax-aware text objects. Keep the user's setup if their kakrc started it.

try %{ nop %opt{tree_sitter_lang} } catch %{
    evaluate-commands %sh{
        command -v kak-tree-sitter >/dev/null 2>&1 && kak-tree-sitter -dks --init "$kak_session"
    }
}

# kak-tree-sitter leaves its ts_* faces at "default", expecting a tree-sitter
# colorscheme; link them to Kakoune's standard faces so any colorscheme works.
declare-option -docstring "link kak-tree-sitter's faces to the colorscheme's standard faces" \
    bool neokak_ts_faces true
define-command -hidden neokak-ts-faces %{
    set-face global ts_attribute attribute
    set-face global ts_comment comment
    set-face global ts_constant value
    set-face global ts_constant_builtin value
    set-face global ts_constant_character value
    set-face global ts_constant_numeric value
    set-face global ts_constructor type
    set-face global ts_error Error
    set-face global ts_function function
    set-face global ts_function_builtin builtin
    set-face global ts_function_macro meta
    set-face global ts_function_method function
    set-face global ts_include keyword
    set-face global ts_keyword keyword
    set-face global ts_keyword_operator operator
    set-face global ts_label meta
    set-face global ts_namespace module
    set-face global ts_operator operator
    set-face global ts_special meta
    set-face global ts_string string
    set-face global ts_string_escape meta
    set-face global ts_string_regexp meta
    set-face global ts_tag keyword
    set-face global ts_type type
    set-face global ts_type_builtin type
    set-face global ts_variable variable
    set-face global ts_variable_builtin builtin
    set-face global ts_variable_other_member variable
    set-face global ts_variable_parameter variable
    set-face global ts_markup_heading header
    set-face global ts_markup_bold +b
    set-face global ts_markup_italic +i
    set-face global ts_markup_link_url link
    set-face global ts_markup_raw mono
}
try %{
    nop %opt{tree_sitter_lang}
    evaluate-commands %sh{ [ "$kak_opt_neokak_ts_faces" = true ] && echo neokak-ts-faces }
    # Object-mode text objects: <a-a>f selects the function around the
    # cursor, <a-a>t the type (struct, class, impl); <a-i> their bodies.
    map global object f '<a-semicolon>tree-sitter-object-text-objects function<ret>' -docstring 'function (tree-sitter)'
    map global object t '<a-semicolon>tree-sitter-object-text-objects class<ret>'    -docstring 'type (tree-sitter)'
}

# Folding
# ‾‾‾‾‾‾‾
# Kakoune has no folding, so folds are drawn with a replace-ranges
# highlighter: a fold's lines collapse into "··· N lines" after its first
# line, and open again while the cursor is inside them.

declare-option -hidden range-specs neokak_folds
declare-option -hidden str neokak_fold_from
declare-option -hidden str neokak_fold_a
declare-option -hidden str neokak_fold_b
set-face global NeokakFold comment
add-highlighter global/neokak-folds replace-ranges neokak_folds
hook global BufCreate .* %{ set-option buffer neokak_folds %val{timestamp} }
evaluate-commands -buffer * %{ set-option buffer neokak_folds %val{timestamp} }

# Folds lines <first> to <last>: the first line stays, the rest collapse.
define-command -hidden neokak-fold-lines -params 2 %{
    evaluate-commands -draft %{
        select "%arg{1}.1,%arg{1}.1"
        execute-keys x
        set-option window neokak_fold_a %val{selection_desc}
        select "%arg{2}.1,%arg{2}.1"
        execute-keys x
        set-option window neokak_fold_b %val{selection_desc}
    }
    evaluate-commands %sh{
        start=${kak_opt_neokak_fold_a##*,}    # the first line's newline
        end=${kak_opt_neokak_fold_b##*,}      # the last line's newline
        end_line=${end%.*}; end_col=$((${end#*.} - 1))
        if [ "$2" -le "$1" ] || [ "$end_col" -lt 1 ]; then echo 'fail nothing to fold'; exit; fi
        echo "set-option -add buffer neokak_folds '$start,$end_line.$end_col|{NeokakFold} ··· $(($2 - $1)) lines'"
        # Keep the cursor on the first line, outside the hidden part.
        echo "select $1.1,$1.1"
    }
}

# Indentation folds: the block indented deeper than the cursor line (or the
# block the cursor is in), minus trailing blank lines.
define-command -hidden neokak-fold-indent %{
    evaluate-commands %sh{
        file=$(mktemp "${TMPDIR:-/tmp}/neokak-fold.XXXXXX")
        echo "evaluate-commands -draft -no-hooks %{ write -force '$file' }"
        echo "neokak-fold-indent-file '$file'"
    }
}
define-command -hidden neokak-fold-indent-file -params 1 %{
    evaluate-commands %sh{
        range=$(awk -v c="$kak_cursor_line" '
            function indent(s) { match(s, /^[ \t]*/); return RLENGTH }
            { blank[NR] = ($0 ~ /^[ \t]*$/); ind[NR] = indent($0) }
            END {
                n = c + 1; while (n <= NR && blank[n]) n++
                if (n <= NR && !blank[c] && ind[n] > ind[c]) h = c
                else {
                    h = c - 1; while (h >= 1 && (blank[h] || ind[h] >= ind[c])) h--
                    if (h < 1) exit
                }
                e = h
                for (i = h + 1; i <= NR; i++) { if (blank[i]) continue; if (ind[i] <= ind[h]) break; e = i }
                if (e > h) print h, e
            }' "$1")
        rm -f "$1"
        if [ -n "$range" ]; then echo "neokak-fold-lines $range"; else echo 'fail no indented block here'; fi
    }
}

# Run by the frontend after it typed <a-a>f for kak-tree-sitter (which
# answers asynchronously): fold the function that got selected, or fall back
# to indentation if there was none.
define-command -hidden neokak-fold-finish %{
    evaluate-commands %sh{
        set -- $kak_selection_desc
        anchor=${1%,*}; cursor=${1#*,}
        a=${anchor%.*}; b=${cursor%.*}
        if [ "$a" != "$b" ]; then
            if [ "$a" -lt "$b" ]; then echo "neokak-fold-lines $a $b"; else echo "neokak-fold-lines $b $a"; fi
        else
            echo 'select %opt{neokak_fold_from}; neokak-fold-indent'
        fi
    }
}

define-command neokak-fold -docstring 'fold the function (tree-sitter) or indented block at the cursor' %{
    set-option window neokak_fold_from %val{selections_desc}
    evaluate-commands %sh{
        if [ -n "$kak_opt_tree_sitter_lang" ]; then echo 'neokak-msg fold-ts'; else echo neokak-fold-indent; fi
    }
}

define-command neokak-unfold -docstring 'open the folds at the cursor line' %{
    evaluate-commands %sh{
        eval set -- "$kak_quoted_opt_neokak_folds"
        ts=$1; shift
        kept=""
        for spec; do
            range=${spec%%|*}; first=${range%%.*}; last=${range#*,}; last=${last%%.*}
            if [ "$kak_cursor_line" -ge "$first" ] && [ "$kak_cursor_line" -le "$last" ]; then continue; fi
            kept="$kept '$(printf %s "$spec" | sed "s/'/''/g")'"
        done
        echo "set-option buffer neokak_folds $ts $kept"
    }
}

define-command neokak-fold-toggle -docstring 'fold or unfold at the cursor' %{
    evaluate-commands %sh{
        eval set -- "$kak_quoted_opt_neokak_folds"
        shift
        for spec; do
            range=${spec%%|*}; first=${range%%.*}; last=${range#*,}; last=${last%%.*}
            if [ "$kak_cursor_line" -ge "$first" ] && [ "$kak_cursor_line" -le "$last" ]; then
                echo neokak-unfold; exit
            fi
        done
        echo neokak-fold
    }
}

define-command neokak-unfold-all -docstring 'open every fold in the buffer' %{
    set-option buffer neokak_folds %val{timestamp}
}

# Function and symbol navigation
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾
# kak-lsp's symbol navigation, or kak-tree-sitter's when there's no LSP.

define-command -hidden neokak-goto-symbol -params 2 %{
    try %{
        nop %opt{lsp_servers}
        evaluate-commands "lsp-%arg{1}-%arg{2}"
    } catch %{
        evaluate-commands %sh{
            if [ "$1" = next ]; then mode=search_next; else mode=search_prev; fi
            echo "tree-sitter-text-objects function.around $mode"
        }
    }
}

# Sessions
# ‾‾‾‾‾‾‾‾
# The files open in each project directory (and the cursor in each) are
# saved as you work, so they can be restored later.

declare-option -docstring 'save the open files of each project directory' bool neokak_session true
declare-option -docstring %{
    restore the project's session when neokak starts without files
} bool neokak_session_autorestore false
declare-option -hidden str neokak_pos 1.1
declare-option -hidden str-list neokak_session_data

hook global NormalIdle .* %{ set-option buffer neokak_pos "%val{cursor_line}.%val{cursor_column}" }

define-command -hidden neokak-session-sync %{
    set-option global neokak_session_data
    evaluate-commands -buffer * %{
        set-option -add global neokak_session_data %val{buffile} %opt{neokak_pos}
    }
    neokak-msg session %opt{neokak_session} %opt{neokak_session_data}
}
hook global WinDisplay .* neokak-session-sync
hook global BufWritePost .* neokak-session-sync
# (Also on ClientClose, from the hook above.)

define-command neokak-session-restore -docstring "reopen the files from this project's last session" %{
    neokak-msg session-restore
}
define-command neokak-session-forget -docstring %{
    neokak-session-forget: delete this project's session and stop saving it until neokak restarts
} %{
    neokak-msg session-forget
}
# Switching projects closes the files from outside the new project, after
# asking about unsaved changes in them.
declare-option -hidden str-list neokak_bufs
declare-option -hidden str neokak_switch_to

# Every buffer's name, file and modified flag, flattened.
define-command -hidden neokak-collect-bufs %{
    set-option global neokak_bufs
    evaluate-commands -buffer * %{
        set-option -add global neokak_bufs %val{bufname} %val{buffile} %val{modified}
    }
}

define-command neokak-session-load -params 1 -docstring %{
    neokak-session-load <dir>: switch to a project directory and restore its session
} %{
    set-option global neokak_switch_to %arg{1}
    neokak-collect-bufs
    evaluate-commands %sh{
        dir=${1%/}
        eval set -- "$kak_quoted_opt_neokak_bufs"
        unsaved=""
        while [ $# -ge 3 ]; do
            name=$1 file=$2 modified=$3; shift 3
            case "$file" in "$dir"/*|''|[!/]*) continue ;; esac
            [ "$modified" = true ] && unsaved="$unsaved
  $name"
        done
        if [ -z "$unsaved" ]; then
            echo neokak-session-switch
            exit
        fi
        text="Unsaved changes in files that switching projects closes:
$unsaved

y: discard them and switch
w: save them all and switch
any other key: stay"
        printf "info -style modal -title 'Unsaved changes' '%s'\n" "$(printf %s "$text" | sed "s/'/''/g")"
        echo 'on-key %{ info -style modal; neokak-session-load-key %val{key} }'
    }
}

define-command -hidden neokak-session-load-key -params 1 %{
    evaluate-commands %sh{
        case "$1" in
            y) echo neokak-session-switch ;;
            w) echo 'write-all; neokak-session-switch' ;;
            *) echo "echo 'Staying in this project'" ;;
        esac
    }
}

define-command -hidden neokak-session-switch %{
    # Save the project being left with its files still open.
    neokak-session-sync
    neokak-msg session-load %opt{neokak_switch_to}
}

# Run by the frontend after changing directory: close the file buffers from
# outside <dir>. Session saving pauses meanwhile, so the new project's
# session never gets a half-closed list.
declare-option -hidden bool neokak_session_was true
define-command -hidden neokak-close-outside -params 1 %{
    neokak-collect-bufs
    set-option global neokak_session_was %opt{neokak_session}
    set-option global neokak_session false
    try %{ evaluate-commands %sh{
        dir=${1%/}
        eval set -- "$kak_quoted_opt_neokak_bufs"
        while [ $# -ge 3 ]; do
            name=$1 file=$2; shift 3
            case "$file" in "$dir"/*|''|[!/]*) continue ;; esac
            printf "delete-buffer! '%s'\n" "$(printf %s "$name" | sed "s/'/''/g")"
        done
    } }
    set-option global neokak_session %opt{neokak_session_was}
    neokak-sync-buflist
    neokak-sync-current
}

# Pickers and the plugin API
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾

define-command neokak-picker -params 1 -docstring %{
    neokak-picker <name>: open a picker
    Built in: files, grep, buffers, recent, projects, git-status, themes, pickers. Plugins add more with
    neokak-picker-define.
} %{
    neokak-msg picker %arg{1}
}

define-command neokak-picker-define -params 4 -docstring %{
    neokak-picker-define <name> <title> <source> <accept>: register a picker
    <source> is a shell command whose output lines are the items.
    <accept> is a Kakoune command; the chosen item is passed as its argument.
} %{
    neokak-msg picker-define %arg{1} %arg{2} %arg{3} %arg{4}
}

define-command neokak-explorer -docstring %{
    neokak-explorer: open and focus the file tree (closes it if focused)
} %{
    neokak-msg tree
}

define-command neokak-window -params 1 -docstring %{
    neokak-window <command>: manage split panes
    vsplit, split, close, only, next, equalize, h/j/k/l to move focus,
    or resize-left/right/up/down.
} %{
    neokak-msg window %arg{1}
}

define-command neokak-notify -params 1..2 -docstring %{
    neokak-notify [<level>] <message>: show a notification toast
    <level> is one of info, warn, error (default info).
} %{
    neokak-msg notify %arg{@}
}

neokak-picker-define colorschemes 'Colorschemes' "ls '%val{runtime}/colors' '%val{config}/colors' 2>/dev/null | sed -n 's/\.kak$//p' | sort -u" colorscheme

# Leader keys (<space>, Kakoune's user mode)
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾

try %{ declare-user-mode find }
try %{ declare-user-mode buffer }
try %{ declare-user-mode window }
try %{ declare-user-mode search }
try %{ declare-user-mode git }
try %{ declare-user-mode session }
try %{ declare-user-mode fold }

map global user <space> ': neokak-picker files<ret>'   -docstring 'find files'
map global user ,       ': neokak-picker buffers<ret>' -docstring 'switch buffer'
map global user e       ': neokak-explorer<ret>'       -docstring 'file explorer'
map global user t       ': neokak-msg terminal<ret>'   -docstring 'terminal (<c-\> hides)'
map global user f       ': enter-user-mode find<ret>'  -docstring '+find'
map global user b       ': enter-user-mode buffer<ret>' -docstring '+buffer'
map global user w       ': enter-user-mode window<ret>' -docstring '+window'
map global user s       ': enter-user-mode search<ret>' -docstring '+search (LSP)'
map global user g       ': enter-user-mode git<ret>'    -docstring '+git'
map global user q       ': enter-user-mode session<ret>' -docstring '+session'
map global user z       ': enter-user-mode fold<ret>'    -docstring '+fold'
map global user j       ': neokak-goto-symbol next function<ret>'     -docstring 'next function'
map global user k       ': neokak-goto-symbol previous function<ret>' -docstring 'previous function'
map global user J       ': neokak-goto-symbol next symbol<ret>'       -docstring 'next symbol'
map global user K       ': neokak-goto-symbol previous symbol<ret>'   -docstring 'previous symbol'
map global user |       ': neokak-window vsplit<ret>'  -docstring 'split right'
map global user <minus> ': neokak-window split<ret>'   -docstring 'split below'

map global find f ': neokak-picker files<ret>'        -docstring 'files'
map global find g ': neokak-picker grep<ret>'         -docstring 'live grep'
map global find b ': neokak-picker buffers<ret>'      -docstring 'buffers'
map global find r ': neokak-picker recent<ret>'       -docstring 'recent files'
map global find c ': neokak-picker colorschemes<ret>' -docstring 'colorschemes'
map global find t ': neokak-picker themes<ret>'       -docstring 'interface themes'
map global find p ': neokak-picker pickers<ret>'      -docstring 'all pickers'

map global buffer b ': neokak-picker buffers<ret>' -docstring 'pick buffer'
map global buffer n ': buffer-next<ret>'           -docstring 'next buffer'
map global buffer p ': buffer-previous<ret>'       -docstring 'previous buffer'
map global buffer d ': delete-buffer<ret>'         -docstring 'delete buffer'
map global buffer D ': delete-buffer!<ret>'        -docstring 'delete buffer (discard changes)'

map global window v ': neokak-window vsplit<ret>' -docstring 'split right'
map global window s ': neokak-window split<ret>'  -docstring 'split below'
map global window h ': neokak-window h<ret>'      -docstring 'focus left'
map global window j ': neokak-window j<ret>'      -docstring 'focus down'
map global window k ': neokak-window k<ret>'      -docstring 'focus up'
map global window l ': neokak-window l<ret>'      -docstring 'focus right'
map global window w ': neokak-window next<ret>'   -docstring 'next pane'
map global window q ': neokak-window close<ret>'  -docstring 'close pane'
map global window o ': neokak-window only<ret>'   -docstring 'close other panes'
map global window <gt>    ': neokak-window resize-right<ret>' -docstring 'wider'
map global window <lt>    ': neokak-window resize-left<ret>'  -docstring 'narrower'
map global window +       ': neokak-window resize-up<ret>'    -docstring 'taller'
map global window <minus> ': neokak-window resize-down<ret>'  -docstring 'shorter'
map global window =       ': neokak-window equalize<ret>'     -docstring 'equal sizes'

map global search s ': lsp-document-symbol<ret>'                           -docstring 'document symbols'
map global search S ': lsp-workspace-symbol '                              -docstring 'workspace symbols (type a query)'
map global search r ': neokak-lsp References lsp-references<ret>'          -docstring 'references'
map global search R ': neokak-lsp context:References lsp-references<ret>'  -docstring 'references with context'
map global search d ': lsp-diagnostics<ret>'                               -docstring 'diagnostics'
map global search i ': neokak-lsp Implementations lsp-implementation<ret>' -docstring 'implementations'
map global search t ': neokak-lsp %{Type Definitions} lsp-type-definition<ret>' -docstring 'type definition'

map global git g ': neokak-float %opt{neokak_git_tool}<ret>' -docstring 'git TUI (lazygit)'
map global git s ': neokak-picker git-status<ret>'           -docstring 'changed files'
map global git n ': git next-hunk<ret>'                      -docstring 'next change'
map global git p ': git prev-hunk<ret>'                      -docstring 'previous change'
map global git b ': neokak-git-blame-toggle<ret>'            -docstring 'toggle line blame'
map global git B ': git blame<ret>'                          -docstring 'blame the whole file'
map global git d ': git diff %val{buffile}<ret>'             -docstring 'diff of this file'

map global session s ': neokak-session-restore<ret>'   -docstring 'restore this project'
map global session p ': neokak-picker projects<ret>'   -docstring 'switch project'
map global session d ': neokak-session-forget<ret>'    -docstring "forget this project's session"

map global fold z ': neokak-fold-toggle<ret>' -docstring 'toggle fold'
map global fold c ': neokak-fold<ret>'        -docstring 'fold'
map global fold o ': neokak-unfold<ret>'      -docstring 'unfold'
map global fold R ': neokak-unfold-all<ret>'  -docstring 'unfold all'

# Ctrl+arrows resize the focused pane (or the explorer), as in LazyVim.
map global normal <c-right> ': neokak-window resize-right<ret>'
map global normal <c-left>  ': neokak-window resize-left<ret>'
map global normal <c-up>    ': neokak-window resize-up<ret>'
map global normal <c-down>  ': neokak-window resize-down<ret>'

# Plugin manager
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾
# In ~/.config/neokak/init.kak:
#   neokak-plug alexherbo2/auto-pairs.kak %{ enable-auto-pairs }
# then run :neokak-plug-install once.

declare-option -hidden str-list neokak_plugins
declare-option -hidden str neokak_plugin_dir %sh{
    printf %s "${XDG_DATA_HOME:-$HOME/.local/share}/neokak/plugins"
}

define-command neokak-plug -params 1..2 -docstring %{
    neokak-plug <repo> [<config>]: declare a plugin
    <repo> is a git URL or a GitHub user/repo. Once installed (with
    :neokak-plug-install), its .kak files are sourced and <config> evaluated.
} %{
    set-option -add global neokak_plugins %arg{1}
    evaluate-commands %sh{
        name=${1##*/}; name=${name%.git}
        var=$(printf %s "$name" | tr -c 'A-Za-z0-9_' '_')
        printf 'declare-option -hidden str neokak_plug_cfg_%s\n' "$var"
        printf 'set-option global neokak_plug_cfg_%s %%arg{2}\n' "$var"
        if [ -d "$kak_opt_neokak_plugin_dir/$name" ]; then
            printf "neokak-plug-load '%s'\n" "$name"
        else
            printf "neokak-msg notify warn 'plugin %s is not installed: run :neokak-plug-install'\n" "$name"
        fi
    }
}

define-command -hidden neokak-plug-load -params 1 %{
    evaluate-commands %sh{
        find -L "$kak_opt_neokak_plugin_dir/$1" -type f -name '*.kak' -not -path '*/.git/*' | sort |
        while IFS= read -r f; do
            q=$(printf %s "$f" | sed "s/'/''/g")
            printf "try %%{ source '%s' } catch %%{ echo -debug %%val{error} }\n" "$q"
        done
        var=$(printf %s "$1" | tr -c 'A-Za-z0-9_' '_')
        printf 'evaluate-commands %%opt{neokak_plug_cfg_%s}\n' "$var"
    }
}

define-command neokak-plug-install -docstring 'neokak-plug-install: clone declared plugins that are missing' %{
    nop %sh{
        eval set -- "$kak_quoted_opt_neokak_plugins"
        dir=$kak_opt_neokak_plugin_dir
        session=$kak_session
        (
            mkdir -p "$dir"
            n=0
            for repo; do
                name=${repo##*/}; name=${name%.git}
                [ -d "$dir/$name" ] && continue
                case $repo in *://*|git@*) url=$repo ;; *) url=https://github.com/$repo ;; esac
                if GIT_TERMINAL_PROMPT=0 git clone --quiet --depth 1 "$url" "$dir/$name"; then
                    printf "neokak-plug-load '%s'\n" "$name" | kak -p "$session"
                    "$NEOKAK_BIN" msg notify info "installed $name"
                    n=$((n + 1))
                else
                    "$NEOKAK_BIN" msg notify error "failed to clone $url"
                fi
            done
            [ "$n" -eq 0 ] && "$NEOKAK_BIN" msg notify info "plugins: nothing to install"
        ) >/dev/null 2>&1 </dev/null &
    }
}

define-command neokak-plug-update -docstring 'neokak-plug-update: git pull every installed plugin' %{
    nop %sh{
        eval set -- "$kak_quoted_opt_neokak_plugins"
        dir=$kak_opt_neokak_plugin_dir
        (
            n=0
            for repo; do
                name=${repo##*/}; name=${name%.git}
                [ -d "$dir/$name/.git" ] || continue
                before=$(git -C "$dir/$name" rev-parse HEAD)
                GIT_TERMINAL_PROMPT=0 git -C "$dir/$name" pull --quiet --ff-only ||
                    "$NEOKAK_BIN" msg notify error "failed to update $name"
                [ "$before" != "$(git -C "$dir/$name" rev-parse HEAD)" ] && n=$((n + 1))
            done
            "$NEOKAK_BIN" msg notify info "plugins: $n updated (restart to reload)"
        ) >/dev/null 2>&1 </dev/null &
    }
}

define-command neokak-plug-clean -docstring 'neokak-plug-clean: delete installed plugins that are no longer declared' %{
    nop %sh{
        eval set -- "$kak_quoted_opt_neokak_plugins"
        dir=$kak_opt_neokak_plugin_dir
        declared=" "
        for repo; do name=${repo##*/}; declared="$declared${name%.git} "; done
        n=0
        for d in "$dir"/*/; do
            [ -d "$d" ] || continue
            name=$(basename "$d")
            case $declared in *" $name "*) ;; *) rm -rf "$d"; n=$((n + 1)) ;; esac
        done
        "$NEOKAK_BIN" msg notify info "plugins: removed $n"
    }
}

# User overrides
# ‾‾‾‾‾‾‾‾‾‾‾‾‾‾

evaluate-commands %sh{
    f="${XDG_CONFIG_HOME:-$HOME/.config}/neokak/init.kak"
    [ -f "$f" ] && printf "source '%s'\n" "$(printf %s "$f" | sed "s/'/''/g")"
}

neokak-sync-current
neokak-sync-buflist
neokak-sync-theme
neokak-sync-env-providers

neokak-msg session-autorestore %opt{neokak_session_autorestore}
neokak-msg ready

# Files named on the command line were opened before this file was sourced.
evaluate-commands -buffer * %{ try %{ nop %val{buffile}; neokak-git-check } }
try neokak-git-show
