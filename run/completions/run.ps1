# PowerShell completion script for run command

    Register-ArgumentCompleter -Native -CommandName run -ScriptBlock {
        param($wordToComplete, $commandAst, $cursorPosition)

        $commandElements = $commandAst.CommandElements
        $numArgs = $commandElements.Count

        function Get-RunFunctions {
            try {
                $listOutput = & run --list 2>$null
                if ($LASTEXITCODE -eq 0 -and $listOutput) {
                    $listOutput | ForEach-Object {
                        $line = $_ -replace ' \(overrides global\)', ''
                        if ($line -match '^\s+(\S+)\s*$') {
                            $matches[1]
                        }
                    }
                }
            } catch {}
        }

        $allFuncs = @(Get-RunFunctions)

        # Collect namespaces
        $namespaces = @{}
        foreach ($func in $allFuncs) {
            if ($func -like '*:*') {
                $prefix = $func.Split(':')[0]
                $namespaces[$prefix] = $true
            }
        }

        # Check if we're completing after a namespace argument
        $prevArg = if ($numArgs -ge 2) { $commandElements[1].ToString() } else { $null }
        $isAfterNamespace = $prevArg -and $namespaces.ContainsKey($prevArg)

        if ($numArgs -ge 3 -or ($numArgs -eq 2 -and $isAfterNamespace)) {
            # Second argument: show subcommands for the namespace
            $namespace = $commandElements[1].ToString()
            foreach ($func in $allFuncs) {
                if ($func -like "$namespace`:*") {
                    $subcmd = $func.Split(':')[1]
                    if ($subcmd -like "$wordToComplete*") {
                        [System.Management.Automation.CompletionResult]::new($subcmd, $subcmd, 'ParameterValue', $subcmd)
                    }
                }
            }
        }
        else {
            # First argument: show top-level commands and namespace prefixes
            $topLevel = @()
            foreach ($func in $allFuncs) {
                if ($func -like '*:*') {
                    $topLevel += $func.Split(':')[0]
                } else {
                    $topLevel += $func
                }
            }

            $topLevel | Sort-Object -Unique | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
                [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
            }
        }
    }