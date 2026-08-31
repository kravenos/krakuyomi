local _ = require("gettext+")

local SourceError = {}

local messages = {
    timeout = _("The source timed out."),
    network = _("The source could not be reached."),
    http = _("The source returned an HTTP error."),
    parse = _("The source returned data that could not be read."),
    source_trap = _("The source stopped while processing the request."),
    incompatible = _("The source is not compatible with this version."),
    missing_source = _("The source is not installed or did not load."),
    internal = _("The source operation could not be completed."),
}

--- Returns the translated safe message for a structured source failure.
--- @param category string|nil
--- @param fallback string|nil
--- @return string
function SourceError.message(category, fallback)
    return messages[category] or fallback or messages.internal
end

--- Formats one source-level batch summary.
--- @param summary table
--- @return string
function SourceError.formatSummary(summary)
    return string.format(
        _("%s: %d failed, %d skipped - %s"),
        summary.source_name or summary.source_id or _("Unknown source"),
        summary.failed or 0,
        summary.skipped or 0,
        SourceError.message(summary.category, summary.message)
    )
end

return SourceError
