@{
    ExcludeRules = @(
        'PSUseApprovedVerbs',       # logging helpers use non-Verb-Noun names by design
        'PSAvoidUsingWriteHost'     # console tool: explicit console output is intended
    )
}

