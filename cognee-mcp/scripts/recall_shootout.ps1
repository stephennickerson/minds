param(
    [string]$DatasetName = "fleet_smoke_api_20260510",
    [string]$DatasetId = "40089743-cf53-50f0-bf25-3ce347e8d6d7",
    [string]$ServiceUrl = "http://localhost:8000",
    [string]$RustExe = ".\target\release\cognee-mcp-rs.exe",
    [string]$CliExe = "..\cognee\.venv\Scripts\cognee-cli.exe",
    [string]$ReadModelPath = ".cognee\read_model.sqlite",
    [string]$OutputPath = ".\target\recall-shootout.md",
    [int]$CliTimeoutSec = 90,
    [int]$ServerTimeoutSec = 90,
    [int]$McpTimeoutSec = 90,
    [int]$QuestionLimit = 0
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-RepoPath {
    param([string]$Path)

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $Path))
}

function Invoke-ProcessTimed {
    param(
        [string]$FileName,
        [string[]]$Arguments,
        [int]$TimeoutSec
    )

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo.FileName = $FileName
    foreach ($argument in $Arguments) {
        [void]$process.StartInfo.ArgumentList.Add($argument)
    }
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.RedirectStandardOutput = $true
    $process.StartInfo.RedirectStandardError = $true
    $process.StartInfo.CreateNoWindow = $true

    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()

    if (-not $process.WaitForExit($TimeoutSec * 1000)) {
        try {
            $process.Kill($true)
        } catch {
            $process.Kill()
        }
        $timer.Stop()
        return [pscustomobject]@{
            ExitCode = -1
            TimedOut = $true
            LatencyMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 2)
            Stdout = ""
            Stderr = "Timed out after $TimeoutSec seconds."
        }
    }

    $timer.Stop()
    return [pscustomobject]@{
        ExitCode = $process.ExitCode
        TimedOut = $false
        LatencyMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 2)
        Stdout = $stdoutTask.GetAwaiter().GetResult()
        Stderr = $stderrTask.GetAwaiter().GetResult()
    }
}

function Read-ByteWithTimeout {
    param(
        [System.IO.Stream]$Stream,
        [datetime]$Deadline
    )

    $buffer = New-Object byte[] 1
    $remainingMs = [int][math]::Max(1, ($Deadline - [datetime]::UtcNow).TotalMilliseconds)
    $task = $Stream.ReadAsync($buffer, 0, 1)
    if (-not $task.Wait($remainingMs)) {
        throw "Timed out waiting for MCP stdout."
    }
    if ($task.Result -eq 0) {
        throw "MCP stdout closed."
    }
    return $buffer[0]
}

function Read-McpLine {
    param(
        [System.IO.Stream]$Stream,
        [datetime]$Deadline
    )

    $bytes = [System.Collections.Generic.List[byte]]::new()
    while ($true) {
        $byte = Read-ByteWithTimeout -Stream $Stream -Deadline $Deadline
        if ($byte -eq 10) {
            break
        }
        if ($byte -ne 13) {
            $bytes.Add($byte)
        }
    }

    return [System.Text.Encoding]::ASCII.GetString($bytes.ToArray())
}

function Read-McpBytes {
    param(
        [System.IO.Stream]$Stream,
        [int]$Length,
        [datetime]$Deadline
    )

    $buffer = New-Object byte[] $Length
    $offset = 0
    while ($offset -lt $Length) {
        if ([datetime]::UtcNow -gt $Deadline) {
            throw "Timed out reading MCP body."
        }
        $remainingMs = [int][math]::Max(1, ($Deadline - [datetime]::UtcNow).TotalMilliseconds)
        $task = $Stream.ReadAsync($buffer, $offset, $Length - $offset)
        if (-not $task.Wait($remainingMs)) {
            throw "Timed out reading MCP body."
        }
        if ($task.Result -eq 0) {
            throw "MCP stdout closed while reading body."
        }
        $offset += $task.Result
    }

    return $buffer
}

function Read-McpFrame {
    param(
        [System.Diagnostics.Process]$Process,
        [int]$TimeoutSec
    )

    $stream = $Process.StandardOutput.BaseStream
    $deadline = [datetime]::UtcNow.AddSeconds($TimeoutSec)
    $contentLength = $null

    while ($true) {
        $line = Read-McpLine -Stream $stream -Deadline $deadline
        if ($line.Length -eq 0) {
            if ($null -eq $contentLength) {
                continue
            }
            break
        }
        $parts = $line.Split(":", 2)
        if ($parts.Length -eq 2 -and $parts[0].Equals("Content-Length", [System.StringComparison]::OrdinalIgnoreCase)) {
            $contentLength = [int]$parts[1].Trim()
        }
    }

    if ($null -eq $contentLength) {
        throw "MCP frame did not include Content-Length."
    }

    $body = Read-McpBytes -Stream $stream -Length $contentLength -Deadline $deadline
    $json = [System.Text.Encoding]::UTF8.GetString($body)
    return $json | ConvertFrom-Json -Depth 100
}

function Send-McpFrame {
    param(
        [System.Diagnostics.Process]$Process,
        [object]$Message
    )

    $json = $Message | ConvertTo-Json -Depth 100 -Compress
    $body = [System.Text.Encoding]::UTF8.GetBytes($json)
    $header = [System.Text.Encoding]::ASCII.GetBytes("Content-Length: $($body.Length)`r`n`r`n")
    $stream = $Process.StandardInput.BaseStream
    $stream.Write($header, 0, $header.Length)
    $stream.Write($body, 0, $body.Length)
    $stream.Flush()
}

function Start-McpProcess {
    param(
        [string]$ExePath,
        [string]$ServiceUrl,
        [string]$ReadModelPath,
        [bool]$OperatorTools = $false
    )

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo.FileName = $ExePath
    [void]$process.StartInfo.ArgumentList.Add("--service-url")
    [void]$process.StartInfo.ArgumentList.Add($ServiceUrl)
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.RedirectStandardInput = $true
    $process.StartInfo.RedirectStandardOutput = $true
    $process.StartInfo.RedirectStandardError = $true
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.Environment["COGNEE_MCP_READ_MODEL_PATH"] = $ReadModelPath
    $process.StartInfo.Environment["COGNEE_MCP_ENABLE_OPERATOR_TOOLS"] = if ($OperatorTools) { "true" } else { "false" }
    [void]$process.Start()
    return $process
}

function Invoke-McpRequest {
    param(
        [System.Diagnostics.Process]$Process,
        [string]$Method,
        [object]$Params,
        [int]$Id,
        [int]$TimeoutSec
    )

    Send-McpFrame -Process $Process -Message @{
        jsonrpc = "2.0"
        id = $Id
        method = $Method
        params = $Params
    }

    while ($true) {
        $response = Read-McpFrame -Process $Process -TimeoutSec $TimeoutSec
        if ($response.PSObject.Properties.Name -contains "id" -and [int]$response.id -eq $Id) {
            return $response
        }
    }
}

function Initialize-Mcp {
    param(
        [System.Diagnostics.Process]$Process,
        [int]$TimeoutSec
    )

    $response = Invoke-McpRequest -Process $Process -Method "initialize" -Id 1 -TimeoutSec $TimeoutSec -Params @{
        protocolVersion = "2024-11-05"
        capabilities = @{}
        clientInfo = @{
            name = "cognee-recall-shootout"
            version = "0.1.0"
        }
    }

    if (-not ($response.PSObject.Properties.Name -contains "result")) {
        throw "MCP initialize failed: $($response | ConvertTo-Json -Depth 20)"
    }

    Send-McpFrame -Process $Process -Message @{
        jsonrpc = "2.0"
        method = "notifications/initialized"
        params = @{}
    }
}

function Invoke-McpRecall {
    param(
        [System.Diagnostics.Process]$Process,
        [int]$Id,
        [string]$Question,
        [string]$DatasetName,
        [bool]$Presummary,
        [int]$TimeoutSec
    )

    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $response = Invoke-McpRequest -Process $Process -Method "tools/call" -Id $Id -TimeoutSec $TimeoutSec -Params @{
            name = "recall"
            arguments = @{
                query = $Question
                datasets = @($DatasetName)
                top_k = 5
                search_type = "GRAPH_COMPLETION"
                llm_presummary = $Presummary
            }
        }
        $timer.Stop()
        $text = ""
        $content = @($response.result.content)
        if ($content.Count -gt 0) {
            $text = [string]$content[0].text
        }
        return [pscustomobject]@{
            ExitCode = 0
            LatencyMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 2)
            Text = $text
            Error = ""
        }
    } catch {
        $timer.Stop()
        return [pscustomobject]@{
            ExitCode = 1
            LatencyMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 2)
            Text = ""
            Error = $_.Exception.Message
        }
    }
}

function Invoke-McpRecallOneShot {
    param(
        [string]$ExePath,
        [string]$ServiceUrl,
        [string]$ReadModelPath,
        [string]$Question,
        [string]$DatasetName,
        [bool]$Presummary,
        [int]$TimeoutSec
    )

    $process = Start-McpProcess -ExePath $ExePath -ServiceUrl $ServiceUrl -ReadModelPath $ReadModelPath -OperatorTools $Presummary
    try {
        Initialize-Mcp -Process $process -TimeoutSec $TimeoutSec
        return Invoke-McpRecall -Process $process -Id 2 -Question $Question -DatasetName $DatasetName -Presummary $Presummary -TimeoutSec $TimeoutSec
    } catch {
        return [pscustomobject]@{
            ExitCode = 1
            LatencyMs = $TimeoutSec * 1000
            Text = ""
            Error = $_.Exception.Message
        }
    } finally {
        if (-not $process.HasExited) {
            try {
                $process.Kill($true)
            } catch {
                $process.Kill()
            }
        }
        $process.Dispose()
    }
}

function Invoke-CliRecall {
    param(
        [string]$CliExe,
        [string]$Question,
        [string]$DatasetName,
        [int]$TimeoutSec
    )

    $result = Invoke-ProcessTimed -FileName $CliExe -TimeoutSec $TimeoutSec -Arguments @(
        "recall",
        $Question,
        "-d",
        $DatasetName,
        "-t",
        "GRAPH_COMPLETION",
        "-k",
        "5",
        "-f",
        "json"
    )

    return [pscustomobject]@{
        ExitCode = $result.ExitCode
        TimedOut = $result.TimedOut
        LatencyMs = $result.LatencyMs
        Text = $result.Stdout
        Error = $result.Stderr
    }
}

function Invoke-ServerRecall {
    param(
        [string]$ServiceUrl,
        [string]$Question,
        [string]$DatasetName,
        [int]$TimeoutSec
    )

    $body = @{
        query = $Question
        datasets = @($DatasetName)
        searchType = "GRAPH_COMPLETION"
        topK = 5
        onlyContext = $false
        verbose = $false
    } | ConvertTo-Json -Depth 20

    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $response = Invoke-WebRequest `
            -Uri "$ServiceUrl/api/v1/recall" `
            -Method Post `
            -Body $body `
            -ContentType "application/json" `
            -TimeoutSec $TimeoutSec `
            -UseBasicParsing
        $timer.Stop()

        return [pscustomobject]@{
            ExitCode = 0
            TimedOut = $false
            LatencyMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 2)
            Text = $response.Content
            Error = ""
        }
    } catch {
        $timer.Stop()
        return [pscustomobject]@{
            ExitCode = 1
            TimedOut = $timer.Elapsed.TotalSeconds -ge $TimeoutSec
            LatencyMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 2)
            Text = ""
            Error = if ($_.ErrorDetails -and $_.ErrorDetails.PSObject.Properties.Name -contains "Message") { $_.ErrorDetails.Message } else { $_.Exception.Message }
        }
    }
}

function Test-TermHits {
    param(
        [string]$Text,
        [string[]]$Terms
    )

    if (-not $Terms -or $Terms.Count -eq 0) {
        return 0
    }

    $lower = $Text.ToLowerInvariant()
    $hits = 0
    foreach ($term in $Terms) {
        if ($lower.Contains($term.ToLowerInvariant())) {
            $hits += 1
        }
    }
    return $hits
}

function Score-RecallText {
    param(
        [string]$Text,
        [string[]]$ExpectedTerms,
        [bool]$NoHit,
        [string]$Mode
    )

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return [pscustomobject]@{
            Correctness = 0
            Grounding = 0
            Completeness = 0
            Usability = 0
            Total = 0
            Notes = "empty output"
        }
    }

    $hitCount = Test-TermHits -Text $Text -Terms $ExpectedTerms
    $correctness = [math]::Min(3, $hitCount)
    if ($NoHit) {
        $correctness = if ($Text -match "0 ranked evidence|No results|\\[\\]|no evidence|not contain") { 3 } else { 1 }
    } elseif ($hitCount -eq 0 -and $Text.Length -gt 120) {
        $correctness = 1
    }

    $grounding = 0
    if ($Text -match "data_id:|node_id:|edge:|Dataset ID|Source / Coverage") {
        $grounding += 2
    }
    if ($Text -match "Ranked evidence|source|citation|handle|dataset") {
        $grounding += 1
    }
    $grounding = [math]::Min(3, $grounding)

    $completeness = 0
    if ($Text.Length -gt 120) {
        $completeness += 1
    }
    if ($hitCount -ge 2 -or $NoHit) {
        $completeness += 1
    }

    $usability = 0
    if ($Text -match "## Answer|## Evidence|Navigate Next") {
        $usability = 2
    } elseif ($Mode -eq "cli" -and $Text.TrimStart().StartsWith("[")) {
        $usability = 1
    } elseif ($Text.Length -gt 0) {
        $usability = 1
    }

    $total = $correctness + $grounding + $completeness + $usability
    $notes = "hits=$hitCount"
    if ($NoHit) {
        $notes = "no-hit handling"
    }

    return [pscustomobject]@{
        Correctness = $correctness
        Grounding = $grounding
        Completeness = $completeness
        Usability = $usability
        Total = $total
        Notes = $notes
    }
}

function Zero-Score {
    param([string]$Note)

    return [pscustomobject]@{
        Correctness = 0
        Grounding = 0
        Completeness = 0
        Usability = 0
        Total = 0
        Notes = $Note
    }
}

function Winner-ForRow {
    param(
        [pscustomobject]$ServerScore,
        [pscustomobject]$RustEvidenceScore,
        [pscustomobject]$RustPresummaryScore,
        [double]$ServerLatency,
        [double]$RustEvidenceLatency,
        [double]$RustPresummaryLatency
    )

    $scores = @(
        [pscustomobject]@{ Name = "rust_evidence"; Score = $RustEvidenceScore.Total; Latency = $RustEvidenceLatency },
        [pscustomobject]@{ Name = "rust_presummary"; Score = $RustPresummaryScore.Total; Latency = $RustPresummaryLatency },
        [pscustomobject]@{ Name = "cognee_server"; Score = $ServerScore.Total; Latency = $ServerLatency }
    )

    return ($scores | Sort-Object -Property @{ Expression = "Score"; Descending = $true }, @{ Expression = "Latency"; Descending = $false } | Select-Object -First 1).Name
}

function Escape-MarkdownCell {
    param([string]$Text)
    return ($Text -replace "\|", "/" -replace "`r?`n", " ").Trim()
}

function Compact-Note {
    param([string]$Text)
    $clean = [regex]::Replace($Text, "`e\[[0-9;]*m", "")
    if ($clean -match "Could not set lock on file[^\r\n]*") {
        return "Cognee CLI failed: $($Matches[0])"
    }
    if ($clean -match "Timed out after [^\r\n]*") {
        return $Matches[0]
    }
    $clean = (Escape-MarkdownCell $clean)
    if ($clean.Length -le 180) {
        return $clean
    }
    return $clean.Substring(0, 180) + "..."
}

function Short-Note {
    param(
        [string]$CliText,
        [string]$RustEvidenceText,
        [string]$Winner
    )

    if ($Winner -eq "cognee_server") {
        if ($RustEvidenceText -notmatch "data_id:|node_id:|edge:") {
            return "Cognee server win: Rust packet lacked a ranked handle for the expected evidence."
        }
        return "Cognee server win: Rust lexical ranking needs review."
    }

    if ($CliText -match "error|traceback|timed out") {
        return "Cognee server baseline failed or timed out."
    }

    return ""
}

$rustExePath = Resolve-RepoPath $RustExe
$readModelFullPath = Resolve-RepoPath $ReadModelPath
$outputFullPath = Resolve-RepoPath $OutputPath

if (-not (Test-Path $rustExePath)) {
    throw "Rust executable not found: $rustExePath. Run cargo build --release first."
}

$questions = @(
    [pscustomobject]@{ Category = "direct fact"; Question = "What is Stephen configuring Cognee to use?"; Expected = @("cognee", "memory", "fleet"); NoHit = $false },
    [pscustomobject]@{ Category = "summary"; Question = "Summarize what the fleet smoke dataset contains."; Expected = @("fleet", "smoke", "dataset"); NoHit = $false },
    [pscustomobject]@{ Category = "relationship"; Question = "How are the Fleet Router and local fastembed embeddings related?"; Expected = @("fleet", "router", "fastembed"); NoHit = $false },
    [pscustomobject]@{ Category = "multi-hop"; Question = "What pipeline result is desired from the configured Fleet Router setup?"; Expected = @("pipeline", "fleet", "router"); NoHit = $false },
    [pscustomobject]@{ Category = "exact phrase"; Question = "Find the exact phrase tiny knowledge graph."; Expected = @("tiny", "knowledge", "graph"); NoHit = $false },
    [pscustomobject]@{ Category = "temporal"; Question = "What happened in the most recent memory in this dataset?"; Expected = @("memory", "dataset", "recent"); NoHit = $false },
    [pscustomobject]@{ Category = "broad ambiguous"; Question = "What should I know about this memory?"; Expected = @("memory", "cognee", "dataset"); NoHit = $false },
    [pscustomobject]@{ Category = "no-hit"; Question = "What does this say about Kubernetes deployment failures?"; Expected = @("kubernetes", "deployment"); NoHit = $true },
    [pscustomobject]@{ Category = "source-specific"; Question = "What does the raw cognee-fleet-smoke source say?"; Expected = @("cognee", "fleet", "smoke"); NoHit = $false },
    [pscustomobject]@{ Category = "operational"; Question = "What should the agent do next with Cognee memory?"; Expected = @("agent", "cognee", "memory"); NoHit = $false }
)

if ($QuestionLimit -gt 0) {
    $questions = @($questions | Select-Object -First $QuestionLimit)
}

$mcp = Start-McpProcess -ExePath $rustExePath -ServiceUrl $ServiceUrl -ReadModelPath $readModelFullPath
$nextId = 2

try {
    Initialize-Mcp -Process $mcp -TimeoutSec $McpTimeoutSec

    $rows = New-Object System.Collections.Generic.List[object]
    foreach ($question in $questions) {
        Write-Host "Benchmarking [$($question.Category)] $($question.Question)"

        $server = Invoke-ServerRecall -ServiceUrl $ServiceUrl -Question $question.Question -DatasetName $DatasetName -TimeoutSec $ServerTimeoutSec

        $rustEvidence = Invoke-McpRecall -Process $mcp -Id $nextId -Question $question.Question -DatasetName $DatasetName -Presummary $false -TimeoutSec $McpTimeoutSec
        $nextId += 1

        $rustPresummary = Invoke-McpRecallOneShot -ExePath $rustExePath -ServiceUrl $ServiceUrl -ReadModelPath $readModelFullPath -Question $question.Question -DatasetName $DatasetName -Presummary $true -TimeoutSec $McpTimeoutSec

        $serverText = if ($server.ExitCode -eq 0) { $server.Text } else { "$($server.Text)`n$($server.Error)" }
        $rustEvidenceText = if ($rustEvidence.ExitCode -eq 0) { $rustEvidence.Text } else { $rustEvidence.Error }
        $rustPresummaryText = if ($rustPresummary.ExitCode -eq 0) { $rustPresummary.Text } else { $rustPresummary.Error }

        $serverScore = Score-RecallText -Text $serverText -ExpectedTerms $question.Expected -NoHit $question.NoHit -Mode "server"
        $rustEvidenceScore = Score-RecallText -Text $rustEvidenceText -ExpectedTerms $question.Expected -NoHit $question.NoHit -Mode "rust_evidence"
        $rustPresummaryScore = Score-RecallText -Text $rustPresummaryText -ExpectedTerms $question.Expected -NoHit $question.NoHit -Mode "rust_presummary"
        if ($server.ExitCode -ne 0) {
            $serverScore = Zero-Score "server failed"
        }
        if ($rustEvidence.ExitCode -ne 0) {
            $rustEvidenceScore = Zero-Score "rust evidence failed"
        }
        if ($rustPresummary.ExitCode -ne 0) {
            $rustPresummaryScore = Zero-Score "rust presummary failed"
        }
        $winner = Winner-ForRow -ServerScore $serverScore -RustEvidenceScore $rustEvidenceScore -RustPresummaryScore $rustPresummaryScore -ServerLatency $server.LatencyMs -RustEvidenceLatency $rustEvidence.LatencyMs -RustPresummaryLatency $rustPresummary.LatencyMs
        $note = Short-Note -CliText $serverText -RustEvidenceText $rustEvidenceText -Winner $winner
        if ($rustEvidence.ExitCode -ne 0) {
            $note = "Rust evidence error: $(Compact-Note $rustEvidence.Error)"
        } elseif ($rustPresummary.ExitCode -ne 0) {
            $note = "Rust presummary error: $(Compact-Note $rustPresummary.Error)"
        } elseif ($server.ExitCode -ne 0) {
            $note = "Cognee server error: $(Compact-Note $server.Error)"
        }

        $rows.Add([pscustomobject]@{
            Category = $question.Category
            Question = $question.Question
            ServerLatencyMs = $server.LatencyMs
            RustEvidenceLatencyMs = $rustEvidence.LatencyMs
            RustPresummaryLatencyMs = $rustPresummary.LatencyMs
            ServerScore = $serverScore.Total
            RustEvidenceScore = $rustEvidenceScore.Total
            RustPresummaryScore = $rustPresummaryScore.Total
            Winner = $winner
            FailureNotes = $note
            ServerExit = $server.ExitCode
            RustEvidenceExit = $rustEvidence.ExitCode
            RustPresummaryExit = $rustPresummary.ExitCode
        })
    }
} finally {
    if (-not $mcp.HasExited) {
        try {
            $mcp.Kill($true)
        } catch {
            $mcp.Kill()
        }
    }
    $mcp.Dispose()
}

$validRows = @($rows | Where-Object { $_.Category -ne "no-hit" -and $_.ServerExit -eq 0 -and $_.RustEvidenceExit -eq 0 })
$serverSuccessRows = @($rows | Where-Object { $_.ServerExit -eq 0 })
$fasterRows = @($validRows | Where-Object { $_.RustEvidenceLatencyMs -lt $_.ServerLatencyMs })
$answerableRows = @($validRows | Where-Object { $_.RustEvidenceScore -ge 7 })
$serverAggregate = ($rows | Measure-Object -Property ServerScore -Sum).Sum
$rustEvidenceAggregate = ($rows | Measure-Object -Property RustEvidenceScore -Sum).Sum
$rustPresummaryAggregate = ($rows | Measure-Object -Property RustPresummaryScore -Sum).Sum
$answerablePercent = if ($validRows.Count -gt 0) { [math]::Round(($answerableRows.Count / $validRows.Count) * 100, 1) } else { 0 }
$allFaster = $validRows.Count -gt 0 -and $fasterRows.Count -eq $validRows.Count
$presummarySameOrBetter = $rustPresummaryAggregate -ge $serverAggregate

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# Cognee Recall Shootout")
$lines.Add("")
$lines.Add("Generated: $(Get-Date -Format o)")
$lines.Add("")
$lines.Add("- Dataset: ``$DatasetName`` / ``$DatasetId``")
$lines.Add("- Service URL: ``$ServiceUrl``")
$lines.Add("- Rust executable: ``$rustExePath``")
$lines.Add("- Cognee baseline: ``POST $ServiceUrl/api/v1/recall``")
$lines.Add("- Read model: ``$readModelFullPath``")
$lines.Add("")
$lines.Add("## Acceptance Summary")
$lines.Add("")
$lines.Add("| Gate | Result |")
$lines.Add("|---|---:|")
$lines.Add("| Cognee server successful baseline rows | $($serverSuccessRows.Count)/$($rows.Count) |")
$lines.Add("| Rust evidence faster than Cognee server on valid questions | $allFaster ($($fasterRows.Count)/$($validRows.Count)) |")
$lines.Add("| Rust evidence answerable from evidence | $answerablePercent% ($($answerableRows.Count)/$($validRows.Count)) |")
$lines.Add("| Rust presummary aggregate >= Cognee server aggregate | $presummarySameOrBetter ($rustPresummaryAggregate vs $serverAggregate) |")
$lines.Add("| Rust evidence aggregate score | $rustEvidenceAggregate |")
$lines.Add("")
$lines.Add("## Results")
$lines.Add("")
$lines.Add("| Category | Question | Cognee server ms | Rust evidence ms | Rust presummary ms | Cognee server score | Rust evidence score | Rust presummary score | Winner | Failure notes |")
$lines.Add("|---|---|---:|---:|---:|---:|---:|---:|---|---|")
foreach ($row in $rows) {
    $lines.Add("| $(Escape-MarkdownCell $row.Category) | $(Escape-MarkdownCell $row.Question) | $($row.ServerLatencyMs) | $($row.RustEvidenceLatencyMs) | $($row.RustPresummaryLatencyMs) | $($row.ServerScore) | $($row.RustEvidenceScore) | $($row.RustPresummaryScore) | $($row.Winner) | $(Escape-MarkdownCell $row.FailureNotes) |")
}
$lines.Add("")
$lines.Add("## Rubric")
$lines.Add("")
$lines.Add("- correctness: 0-3")
$lines.Add("- grounding/citations/handles: 0-3")
$lines.Add("- completeness: 0-2")
$lines.Add("- agent usability: 0-2")
$lines.Add("")
$lines.Add("Heuristic scores are deterministic smoke scores. Review any CLI win manually using the failure notes and raw command output.")

$outputDir = Split-Path -Parent $outputFullPath
if ($outputDir) {
    New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
}
$lines -join "`r`n" | Set-Content -Path $outputFullPath -Encoding UTF8
Write-Host "Wrote $outputFullPath"
