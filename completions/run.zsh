#compdef run

_run() {
    # Disable history expansion to avoid picking up shell history
    setopt localoptions nobanghist

    local context state state_descr line
    typeset -A opt_args

    local cache_file="/tmp/.run_comp_${USER}_${PWD:gs/\//_}"

    # Get all functions from Runfile
    local -a all_funcs
    local run_cmd
    run_cmd=$(whence -p run 2>/dev/null) || run_cmd=$(which run 2>/dev/null) || run_cmd="run"

    if [[ -n "$run_cmd" ]]; then
        local list_output
        list_output=$($run_cmd --list 2>/dev/null)
        if [[ $? -eq 0 && -n "$list_output" ]]; then
            all_funcs=("${(@f)$(echo $list_output | command sed -n 's/^  //p')}")
        fi
    fi

    # Build top-level completions
    local -a top_level_commands
    local -A namespaces

    for func in $all_funcs; do
        if [[ $func == *:* ]]; then
            local prefix="${func%%:*}"
            namespaces[$prefix]=1
        else
            top_level_commands+=($func)
        fi
    done

    # Add namespace prefixes to top-level commands
    top_level_commands+=("${(@k)namespaces}")

    # Check if we're completing a second argument and first arg is a namespace
    if [[ $CURRENT -eq 2 ]]; then
        # Only show commands from the Runfile, not CLI options
        _describe -t commands 'command' top_level_commands

    elif [[ $CURRENT -eq 3 ]]; then
        local namespace="${words[2]}"
        local -a subcommands

        # Find subcommands for this namespace
        for func in $all_funcs; do
            if [[ $func == ${namespace}:* ]]; then
                local subcmd="${func#*:}"
                subcommands+=($subcmd)
            fi
        done

        if [[ ${#subcommands[@]} -gt 0 ]]; then
            _describe -t subcommands 'subcommand' subcommands
        else
            # Not a namespace, might be a function that takes arguments
            _files
        fi
    else
        # For arguments beyond the second position
        _files
    fi
}

_run "$@"
