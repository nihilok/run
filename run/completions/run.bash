#!/usr/bin/env bash
# Bash completion script for run command

_run_complete() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    # Basic options
    opts="--list --generate-completion --install-completion --version --help -l -h"

    # If we're completing after --generate-completion or --install-completion, suggest shells
    if [[ "${prev}" == "--generate-completion" ]] || [[ "${prev}" == "--install-completion" ]]; then
        COMPREPLY=( $(compgen -W "bash zsh fish" -- "${cur}") )
        return 0
    fi

    # If the previous word is a flag, let normal completion happen
    if [[ "${prev}" == -* ]]; then
        return 0
    fi

    # First argument: show top-level commands and flags
    if [[ ${COMP_CWORD} -eq 1 ]]; then
        # Use cached functions if available and recent (< 5 seconds old)
        local cache_key="${PWD}"
        local cache_hash=""
        if command -v md5sum >/dev/null 2>&1; then
            cache_hash=$(echo "$cache_key" | md5sum | cut -d' ' -f1)
        elif command -v md5 >/dev/null 2>&1; then
            cache_hash=$(echo "$cache_key" | md5 | awk '{print $NF}')
        elif command -v shasum >/dev/null 2>&1; then
            cache_hash=$(echo "$cache_key" | shasum | cut -d' ' -f1)
        else
            # Fallback: use sanitized path (not cryptographically strong, but unique-ish)
            cache_hash=$(echo "$cache_key" | tr '/\\' '__')
        fi
        local cache_file="${TMPDIR:-/tmp}/.run_completion_cache_${USER}_${cache_hash}"
        local completions=""

        # Check if cache exists and is less than 5 seconds old
        if [[ -f "$cache_file" ]]; then
            # Prefer stat for clarity; fallback to find with explained magic number
            if command -v stat >/dev/null 2>&1; then
                if stat --version >/dev/null 2>&1 2>/dev/null; then
                    # GNU stat
                    file_mtime=$(stat -c %Y "$cache_file")
                else
                    # BSD stat
                    file_mtime=$(stat -f %m "$cache_file")
                fi
                now=$(date +%s)
                age=$((now - file_mtime))
                if [[ $age -lt 5 ]]; then
                    completions=$(cat "$cache_file" 2>/dev/null)
                fi
            # Fallback: use find with magic number (0.00006 days ≈ 5 seconds)
            elif find "$cache_file" -type f -mtime -0.00006 >/dev/null 2>&1; then
                # 0.00006 days = 5.184 seconds; used to check if file is <5s old
                completions=$(cat "$cache_file" 2>/dev/null)
            fi
        fi

        # Only show top-level commands from the Runfile
        COMPREPLY=( $(compgen -W "${completions}" -- "${cur}") )

    # Second argument: if prev is a namespace, show subcommands
    elif [[ ${COMP_CWORD} -eq 2 ]]; then
        local namespace="$prev"

        if command -v run &> /dev/null; then
            local all_funcs=$(run --list 2>/dev/null | sed 's/ (overrides global)//' | sed -n 's/^  *\([^ ][^ ]*\) *$/\1/p')
            local subcommands=""

            while IFS= read -r func; do
                if [[ $func == ${namespace}:* ]]; then
                    # Extract part after colon
                    local subcmd="${func#*:}"
                    subcommands="${subcommands}${subcmd} "
                fi
            done <<< "$all_funcs"

            if [[ -n "$subcommands" ]]; then
                COMPREPLY=( $(compgen -W "${subcommands}" -- "${cur}") )
            fi
        fi
    fi

    return 0
}

complete -F _run_complete run
