@{
    ExcludeRules = @(
        'PSUseApprovedVerbs',       # logging helpers use non-Verb-Noun names by design
        'PSAvoidUsingWriteHost',    # console tool: explicit console output is intended
        'PSReviewUnusedParameter',  # top-level script params are consumed across nested scopes
        'PSUseShouldProcessForStateChangingFunctions', # small helper funcs guarded by explicit choices
        'PSShouldProcess'           # handled at script scope or by design
    )
}
