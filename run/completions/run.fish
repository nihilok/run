# Fish completion script for run command

# Helper function to get top-level Runfile functions/namespaces with caching
function __run_get_top_level
    # Create cache file based on current directory
    set -l cache_file /tmp/.run_completion_cache_(echo $PWD | string replace -a / _)_(id -u)

    # Check if cache exists and is less than 5 seconds old
    if test -f $cache_file
        # 5 seconds in days for find -mtime: 5/(60*60*24) = 0.00006
        set -l MTIME_5S -0.00006
        # Use find to check if the cache file was modified within the last 5 seconds (portable)
        if find $cache_file -type f -mtime $MTIME_5S 1>/dev/null 2>&1
            cat $cache_file 2>/dev/null
            return 0
        end
    end

    # Get all functions and extract top-level names
    set -l all_funcs (run --list 2>/dev/null | string replace ' (overrides global)' '' | string match -r '^\s+\S+$' | string trim)
    set -l top_level
    set -l seen

    for func in $all_funcs
        if string match -q '*:*' $func
            # Extract prefix before colon
            set -l prefix (string split -m 1 ':' $func)[1]
            if not contains $prefix $seen
                set -a top_level $prefix
                set -a seen $prefix
            end
        else
            # Non-nested function
            set -a top_level $func
        end
    end

    # Cache and output
    printf '%s\n' $top_level | tee $cache_file 2>/dev/null
end

# Helper function to get subcommands for a namespace
function __run_get_subcommands
    set -l namespace $argv[1]
    set -l all_funcs (run --list 2>/dev/null | string replace ' (overrides global)' '' | string match -r '^\s+\S+$' | string trim)

    for func in $all_funcs
        if string match -q "$namespace:*" $func
            # Extract part after colon
            string split -m 1 ':' $func | tail -n 1
        end
    end
end

# Completions for run command
complete -c run -f

# Shell suggestions for --generate-completion and --install-completion flags
complete -c run -l generate-completion -xa 'bash zsh fish powershell'
complete -c run -l install-completion -xa 'bash zsh fish powershell'

# Top-level function/namespace completions (only for the first argument)
complete -c run -n "__fish_is_first_arg" -a "(__run_get_top_level)"

# Subcommand completions (for the second argument if first was a namespace)
complete -c run -n "not __fish_is_first_arg; and __fish_is_nth_token 2" -a "(__run_get_subcommands (commandline -opc)[2])"
