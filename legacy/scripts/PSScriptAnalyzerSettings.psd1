@{
    ExcludeRules = @(
        'PSUseApprovedVerbs',       # logging helpers use non-Verb-Noun names by design
        'PSAvoidUsingWriteHost',    # console tool: explicit console output is intended
        'PSReviewUnusedParameter',  # top-level script params are consumed across nested scopes
        'PSUseShouldProcessForStateChangingFunctions', # small helper funcs guarded by explicit choices
        'PSShouldProcess',          # handled at script scope or by design
        'PSUseBOMForUnicodeEncodedFile', # UTF-8 BOM is added at build time; false positive in analysis
        'PSUseDeclaredVarsMoreThanAssignments', # false positive: vars used in conditional checks
        'PSAvoidUsingEmptyCatchBlock' # silent fail is intentional for non-critical operations
    )
}
