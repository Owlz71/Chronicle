[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, HelpMessage = "New version, for example 1.3.4 or 1.3.4-beta")]
    [string]$Version,

    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

if ($Version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z][0-9A-Za-z.\-]*)?$') {
    throw "Invalid version '$Version'. Expected MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH-suffix, for example 1.3.4 or 1.3.4-beta."
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot

# Every build-critical version field. Documentation (README.md, docs/changelog.md)
# is intentionally excluded and must be updated by hand.
$targets = @(
    @{
        Path    = "Cargo.toml"
        Pattern = '(?s)(\[workspace\.package\].*?version\s*=\s*")[^"]+(")'
        Count   = 1
        Label   = "Rust workspace"
    },
    @{
        Path    = "apps/desktop/package.json"
        Pattern = '(?m)^(\s*"version":\s*")[^"]+(",)'
        Count   = 1
        Label   = "Desktop package manifest"
    },
    @{
        Path    = "apps/desktop/package-lock.json"
        Pattern = '(?m)^(\s*"version":\s*")[^"]+(",)'
        Count   = 2
        Label   = "Desktop lock file"
    },
    @{
        Path    = "apps/desktop/src-tauri/Cargo.toml"
        Pattern = '(?s)(\[package\].*?version\s*=\s*")[^"]+(")'
        Count   = 1
        Label   = "Tauri crate manifest"
    },
    @{
        Path    = "apps/desktop/src-tauri/tauri.conf.json"
        Pattern = '(?m)^(\s*"version":\s*")[^"]+(",)'
        Count   = 1
        Label   = "Tauri configuration"
    }
)

$pending = @()
foreach ($target in $targets) {
    $fullPath = Join-Path $repositoryRoot $target.Path
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        throw "Required file is missing: $($target.Path)"
    }

    $original = Get-Content -LiteralPath $fullPath -Raw
    $regex = [regex]$target.Pattern
    $found = $regex.Matches($original)
    if ($found.Count -lt $target.Count) {
        throw "Expected at least $($target.Count) version field(s) in $($target.Path) but found $($found.Count)."
    }

    # Replace from the last match backwards so earlier indices stay valid.
    $builder = [System.Text.StringBuilder]::new($original)
    $edits = @()
    for ($index = $target.Count - 1; $index -ge 0; $index--) {
        $match = $found[$index]
        $replacement = $match.Groups[1].Value + $Version + $match.Groups[2].Value
        [void]$builder.Remove($match.Index, $match.Length)
        [void]$builder.Insert($match.Index, $replacement)
        # The replaced value sits exactly between capture group 1 and group 2.
        $start = $match.Groups[1].Length
        $length = $match.Length - $match.Groups[1].Length - $match.Groups[2].Length
        $edits += [pscustomobject]@{
            Before = $match.Value.Substring($start, $length)
            After  = $Version
        }
    }

    $pending += [pscustomobject]@{
        Label    = $target.Label
        Path     = $target.Path
        FullPath = $fullPath
        Content  = $builder.ToString()
        Edits    = ($edits | Sort-Object -Property Before)
    }
}

Write-Output "Target version: $Version"
Write-Output ""
foreach ($item in $pending) {
    Write-Output ("{0}  ({1})" -f $item.Label, $item.Path)
    foreach ($edit in $item.Edits) {
        Write-Output ("  version {0}  ->  {1}" -f $edit.Before, $edit.After)
    }
}

if ($DryRun) {
    Write-Output ""
    Write-Output "Dry run: no file was written."
    return
}

$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
foreach ($item in $pending) {
    [System.IO.File]::WriteAllText($item.FullPath, $item.Content, $utf8NoBom)
}

Write-Output ""
Write-Output "Updated $($pending.Count) files."
Write-Output ""
Write-Output "Next steps:"
Write-Output "  1. Update the release line in README.md."
Write-Output "  2. Add a '## v$Version' section at the top of docs/changelog.md, including the download file names."
Write-Output "  3. Run the test suites, then tag with 'v$Version'."
