# Imports the installed Playnite library into Beanbag: games, art, start settings and stats.
# After this, Beanbag needs nothing from Playnite. Safe to run again: art copies only when
# it changed, and stats keep the higher of Beanbag's and Playnite's numbers.
#
#   powershell -ExecutionPolicy Bypass -File scripts\import-playnite.ps1

param(
    [string]$Out = "$env:LOCALAPPDATA\com.beanbag.launcher",
    [string]$PlayniteApp = "$env:LOCALAPPDATA\Playnite",
    [string]$PlayniteData = "$env:APPDATA\Playnite"
)

$ErrorActionPreference = 'Stop'

if (Get-Process Playnite.DesktopApp, Playnite.FullscreenApp -ErrorAction SilentlyContinue) {
    throw 'Exit Playnite first: it locks its database while it runs.'
}

$STEAM = 'cb91dfc9-b977-43bf-8e70-55f46e410fab'
$EPIC = '00000002-dbd1-46c6-b5d0-b1ba559d10e4'
$BATTLENET = 'e3c26a3d-d695-4cb7-a769-5ff7612c7edd'

# Read copies, so Playnite's own files are never opened for writing.
$work = Join-Path $env:TEMP 'beanbag-import'
New-Item -ItemType Directory -Force $work | Out-Null
foreach ($name in 'games', 'sources', 'platforms', 'genres', 'categories') {
    Copy-Item "$PlayniteData\library\$name.db" "$work\$name.db" -Force
}
Add-Type -Path "$PlayniteApp\LiteDB.dll"

function Read-Collection($name) {
    $db = New-Object LiteDB.LiteDatabase("Filename=$work\$name.db;Mode=ReadOnly")
    try {
        $collection = $db.GetCollection(($db.GetCollectionNames() | Select-Object -First 1))
        return @($collection.FindAll())
    } finally {
        $db.Dispose()
    }
}

# A field that is absent reads as $null in PowerShell, not as BsonValue.Null.
function Empty($bson) { $null -eq $bson -or $bson.IsNull }
function Str($bson) { if (Empty $bson) { '' } else { $bson.AsString } }
function Guid($bson) { if (Empty $bson) { '' } else { $bson.AsGuid.ToString() } }
function Num($bson) { if (Empty $bson) { [int64]0 } else { [int64]$bson.RawValue } }
function Bool($bson) { if (Empty $bson) { $false } else { $bson.AsBoolean } }
function Unix($bson) { if (Empty $bson) { [int64]0 } else { ([DateTimeOffset]$bson.AsDateTime).ToUnixTimeSeconds() } }

$names = @{}
foreach ($db in 'sources', 'platforms', 'genres', 'categories') {
    foreach ($row in Read-Collection $db) { $names[(Guid $row['_id'])] = Str $row['Name'] }
}

function Names-Of($ids) {
    $out = New-Object System.Collections.ArrayList
    if (-not (Empty $ids)) {
        foreach ($id in $ids.AsArray) { $n = $names[(Guid $id)]; if ($n) { [void]$out.Add($n) } }
    }
    return ,$out.ToArray()
}

# Store launchers.
$battlenetExe = @(
    (Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Battle.net' -ErrorAction SilentlyContinue).InstallLocation,
    (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -ErrorAction SilentlyContinue).'Battle.net'
) | Where-Object { $_ } | ForEach-Object {
    $p = ($_ -replace '"', '' -replace '\s+--.*$', '')
    if (Test-Path -LiteralPath $p -PathType Container) { Join-Path $p 'Battle.net.exe' } else { $p }
} | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1

$epicApps = @{}
Get-ChildItem 'C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item' -ErrorAction SilentlyContinue | ForEach-Object {
    $m = Get-Content $_.FullName -Raw | ConvertFrom-Json
    $epicApps[$m.AppName] = "$($m.CatalogNamespace)%3A$($m.CatalogItemId)%3A$($m.AppName)"
}

function Expand($text, $game, $installDir) {
    if (-not $text) { return '' }
    $leaf = if ($installDir) { Split-Path $installDir -Leaf } else { '' }
    $text = $text.Replace('{InstallDir}', $installDir).Replace('{InstallDirName}', $leaf)
    $text.Replace('{Name}', $game.name).Replace('{PlayniteDir}', $PlayniteApp)
}

function Get-Launch($row, $game, $installDir) {
    $plugin = Guid $row['PluginId']
    $gameId = Str $row['GameId']
    # A List, because += would split a Playnite action into its fields.
    $actions = New-Object System.Collections.ArrayList
    if (-not (Empty $row['GameActions'])) {
        foreach ($a in $row['GameActions'].AsArray) { if (Bool $a['IsPlayAction']) { [void]$actions.Add($a) } }
    }
    $own = -not (Bool $row['IncludeLibraryPluginAction']) -or $plugin -eq '00000000-0000-0000-0000-000000000000'

    if ($actions.Count -and $own) {
        $a = $actions[0]
        $type = Str $a['Type']
        $path = Expand (Str $a['Path']) $game $installDir
        if ($type -eq 'URL') { return @{ url = $path } }
        if ($type -eq 'File') {
            if ($path -and -not [IO.Path]::IsPathRooted($path)) { $path = Join-Path $installDir $path }
            $dir = Expand (Str $a['WorkingDir']) $game $installDir
            if (-not $dir) { $dir = Split-Path $path -Parent }
            return @{ exe = $path; args = (Expand (Str $a['Arguments']) $game $installDir); workingDir = $dir }
        }
        Write-Warning "$($game.name): '$type' play actions are not supported; skipped."
        return $null
    }
    switch ($plugin) {
        $STEAM { return @{ url = "steam://rungameid/$gameId" } }
        $EPIC {
            $id = if ($epicApps.ContainsKey($gameId)) { $epicApps[$gameId] } else { $gameId }
            return @{ url = "com.epicgames.launcher://apps/$id`?action=launch&silent=true" }
        }
        $BATTLENET {
            if (-not $battlenetExe) { Write-Warning "$($game.name): Battle.net.exe not found; skipped."; return $null }
            return @{ exe = $battlenetExe; args = "--exec=`"launch $gameId`""; workingDir = (Split-Path $battlenetExe -Parent) }
        }
    }
    Write-Warning "$($game.name): no way to start it; skipped."
    return $null
}

function Copy-Art($relative, $id, $kind) {
    if (-not $relative) { return $null }
    $src = if ([IO.Path]::IsPathRooted($relative)) { $relative } else { Join-Path "$PlayniteData\library\files" $relative }
    if (-not (Test-Path -LiteralPath $src)) { return $null }
    $file = "$kind$([IO.Path]::GetExtension($src).ToLower())"
    $dir = Join-Path $Out "art\$id"
    $dest = Join-Path $dir $file
    New-Item -ItemType Directory -Force $dir | Out-Null
    $s = Get-Item -LiteralPath $src
    $d = Get-Item -LiteralPath $dest -ErrorAction SilentlyContinue
    if (-not $d -or $d.Length -ne $s.Length -or $d.LastWriteTimeUtc -lt $s.LastWriteTimeUtc) {
        Copy-Item -LiteralPath $src -Destination $dest -Force
    }
    return "art/$id/$file"
}

function Get-PlainText($html, $max) {
    if (-not $html) { return '' }
    $t = $html -replace '<br\s*/?>', ' ' -replace '</(p|li|h\d)>', ' ' -replace '<[^>]+>', ''
    $t = ([System.Net.WebUtility]::HtmlDecode($t) -replace '\s+', ' ').Trim()
    if ($t.Length -gt $max) { $t = $t.Substring(0, $max).TrimEnd() + [char]0x2026 }
    return $t
}

function Write-Json($path, $value) {
    $tmp = "$path.tmp"
    [IO.File]::WriteAllText($tmp, ($value | ConvertTo-Json -Depth 6 -Compress), (New-Object Text.UTF8Encoding $false))
    Move-Item -LiteralPath $tmp -Destination $path -Force
}

New-Item -ItemType Directory -Force $Out | Out-Null
$statsPath = Join-Path $Out 'stats.json'
$stats = @{}
if (Test-Path $statsPath) {
    (Get-Content $statsPath -Raw | ConvertFrom-Json).PSObject.Properties | ForEach-Object { $stats[$_.Name] = $_.Value }
}

$games = New-Object System.Collections.ArrayList
$skipped = 0
foreach ($row in Read-Collection 'games') {
    if (-not (Bool $row['IsInstalled']) -or (Bool $row['Hidden'])) { continue }
    $id = Guid $row['_id']
    $name = Str $row['Name']
    $installDir = Str $row['InstallDirectory']
    $game = [ordered]@{ id = $id; name = $name }

    $launch = Get-Launch $row $game $installDir
    if (-not $launch) { $skipped++; continue }

    $platform = ''
    if (-not (Empty $row['PlatformIds']) -and $row['PlatformIds'].AsArray.Count) { $platform = $names[(Guid $row['PlatformIds'].AsArray[0])] }
    $sorting = Str $row['SortingName']
    $plugin = Guid $row['PluginId']

    $game.sortName = if ($sorting) { $sorting } else { $name }
    $game.source = $names[(Guid $row['SourceId'])]
    if (-not $game.source) { $game.source = '' }
    $game.platform = if ($platform) { $platform } else { '' }
    $game.description = Get-PlainText (Str $row['Description']) 420
    $game.sizeBytes = Num $row['InstallSize']
    $game.installDir = $installDir
    $game.genres = Names-Of $row['GenreIds']
    $game.categories = Names-Of $row['CategoryIds']
    $game.launch = $launch
    if ($plugin -eq $STEAM) { $game.steamAppId = [int](Str $row['GameId']) }
    $logo = Join-Path $PlayniteData "ExtraMetadata\games\$id\Logo.png"
    $game.art = [ordered]@{
        cover      = Copy-Art (Str $row['CoverImage']) $id 'cover'
        background = Copy-Art (Str $row['BackgroundImage']) $id 'background'
        logo       = Copy-Art $(if (Test-Path -LiteralPath $logo) { $logo }) $id 'logo'
    }
    [void]$games.Add($game)

    $last = Unix $row['LastActivity']
    $old = $stats[$id]
    $stats[$id] = [ordered]@{
        lastPlayed = [math]::Max($last, [int64]$old.lastPlayed)
        playtime   = [math]::Max((Num $row['Playtime']), [int64]$old.playtime)
        playCount  = [math]::Max((Num $row['PlayCount']), [int64]$old.playCount)
    }
}

Write-Json (Join-Path $Out 'library.json') ([ordered]@{
    version    = 1
    importedAt = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    games      = $games.ToArray()
})
Write-Json $statsPath $stats

$art = (Get-ChildItem (Join-Path $Out 'art') -Recurse -File | Measure-Object Length -Sum)
"Imported $($games.Count) games ($skipped skipped) into $Out"
"Art: $($art.Count) files, $([math]::Round($art.Sum / 1MB)) MB"
