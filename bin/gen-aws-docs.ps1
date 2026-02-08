# bin/gen-aws-docs.ps1 -- generate AWS reference docs from live CLI help (Windows)
#
# Sources: aws <service> help, cdk --help, sam --help, terraform --help
# Output:  ~/.config/swebash/docs/aws/{services_reference,iac_patterns,troubleshooting}.md
#
# Usage:   .\sbh gen-aws-docs          (default services)
#          .\bin\gen-aws-docs.ps1      (direct invocation)

param(
    [switch]$Install,
    [Alias("y")]
    [switch]$Yes,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

# Configuration
$OutputDir = if ($env:SWEBASH_AWS_DOCS_DIR) { $env:SWEBASH_AWS_DOCS_DIR } else { "$env:USERPROFILE\.config\swebash\docs\aws" }

# AWS CLI installer URL (override for mirrors/proxies/specific versions)
$AwsCliUrl = if ($env:SWEBASH_AWS_CLI_URL_WINDOWS) {
    $env:SWEBASH_AWS_CLI_URL_WINDOWS
} else {
    "https://awscli.amazonaws.com/AWSCLIV2.msi"
}

# Services to document (order determines output order)
$Services = @(
    "ec2", "s3", "lambda", "iam", "cloudformation",
    "ecs", "eks", "rds", "dynamodb",
    "sqs", "sns", "cloudwatch", "route53"
)

# Per-file character budget (12k tokens ~ 48k chars; split across 3 files)
$CharBudget = 15000

# --- Helpers ---

function Test-Command {
    param([string]$Name)
    $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-AwsHelp {
    param([string]$Service)
    $env:AWS_PAGER = ""
    try {
        & aws $Service help 2>$null | Out-String
    } catch {
        ""
    }
}

function Remove-Formatting {
    param([string]$Text)
    # Remove backspace-based bold/underline and ANSI escapes
    $Text = $Text -replace '.\x08', ''
    $Text = $Text -replace '\x1b\[[0-9;]*m', ''
    $Text = $Text -replace '\r', ''
    $Text
}

function Get-Synopsis {
    param([string]$Raw)
    $synopsis = ""
    $description = ""
    $inSection = ""

    foreach ($line in $Raw -split "`n") {
        switch -Regex ($line) {
            '^SYNOPSIS' { $inSection = "synopsis" }
            '^DESCRIPTION' { $inSection = "description" }
            '^(AVAILABLE COMMANDS|AVAILABLE SUBCOMMANDS|OPTIONS|EXAMPLES|SEE ALSO|OUTPUT|GLOBAL FLAGS|GLOBAL OPTIONS)' {
                if ($inSection -eq "description") { break }
                $inSection = ""
            }
        }
        switch ($inSection) {
            "synopsis" { $synopsis += "$line`n" }
            "description" { $description += "$line`n" }
        }
    }

    # Trim and truncate description
    $descLines = ($description -split "`n" | Select-Object -First 8) -join "`n"
    "$synopsis`n$descLines"
}

function Get-Subcommands {
    param([string]$Raw)
    $inSection = ""
    $commands = @()

    foreach ($line in $Raw -split "`n") {
        switch -Regex ($line) {
            '^(AVAILABLE COMMANDS|AVAILABLE SUBCOMMANDS)' { $inSection = "commands" }
            '^(SYNOPSIS|DESCRIPTION|OPTIONS|EXAMPLES|SEE ALSO|OUTPUT|GLOBAL)' {
                if ($inSection -eq "commands") { break }
            }
        }
        if ($inSection -eq "commands") {
            if ($line -match '^\s*[o*]\s+([a-z][-a-z0-9]+)') {
                $cmd = $Matches[1]
                if ($cmd -ne "help") {
                    $commands += $cmd
                }
            }
        }
    }

    $commands | Select-Object -First 15
}

# --- AWS CLI Installer ---

function Install-AwsCli {
    param([switch]$NonInteractive)

    Write-Host "==> Installing AWS CLI v2 for Windows..."

    $msiUrl = $AwsCliUrl
    $msiPath = "$env:TEMP\AWSCLIV2.msi"

    Write-Host "==> Downloading from $msiUrl..."
    try {
        Invoke-WebRequest -Uri $msiUrl -OutFile $msiPath -UseBasicParsing
    } catch {
        Write-Error "Failed to download AWS CLI installer: $_"
        return $false
    }

    Write-Host "==> Running MSI installer..."
    $installArgs = "/i `"$msiPath`" /passive"
    if ($NonInteractive) {
        $installArgs = "/i `"$msiPath`" /qn"  # Fully silent
    }

    $process = Start-Process -FilePath "msiexec.exe" -ArgumentList $installArgs -Wait -PassThru
    Remove-Item $msiPath -Force -ErrorAction SilentlyContinue

    if ($process.ExitCode -ne 0) {
        Write-Error "MSI installer failed with exit code $($process.ExitCode)"
        return $false
    }

    # Refresh PATH
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = "$machinePath;$userPath"

    # Verify
    if (Test-Command "aws") {
        $version = & aws --version 2>&1
        Write-Host "==> Installed: $version"
        return $true
    } else {
        Write-Host "==> Installation complete. Restart terminal if 'aws' not found."
        return $true
    }
}

# --- Generate services_reference.md ---

function New-ServicesReference {
    if (-not (Test-Command "aws")) {
        Write-Error "AWS CLI not found"
        return $null
    }

    $awsVersion = & aws --version 2>&1 | Select-Object -First 1

    $out = "# AWS CLI Services Reference`n"
    $out += "<!-- Generated by bin/gen-aws-docs.ps1 from: $awsVersion -->`n`n"

    foreach ($svc in $Services) {
        Write-Host "  $svc..." -NoNewline
        $raw = Get-AwsHelp $svc | Remove-Formatting
        if (-not $raw) {
            Write-Host " (skipped)"
            continue
        }
        Write-Host ""

        $upperSvc = $svc.ToUpper()
        $out += "## $upperSvc`n`n"

        # Synopsis
        $synopsis = Get-Synopsis $raw
        if ($synopsis.Trim()) {
            $out += "```````n$synopsis`n```````n`n"
        }

        # Key subcommands
        $subcmds = Get-Subcommands $raw
        if ($subcmds) {
            $out += "**Key commands:**`n"
            foreach ($cmd in $subcmds) {
                $out += "- ``aws $svc $cmd```n"
            }
            $out += "`n"
        }

        # Budget guard
        if ($out.Length -gt $CharBudget) {
            $out += "`n*Truncated to stay within budget.*`n"
            break
        }
    }

    $out
}

# --- Generate iac_patterns.md ---

function New-IacPatterns {
    $out = "# Infrastructure as Code Patterns`n"
    $out += "<!-- Generated by bin/gen-aws-docs.ps1 -->`n`n"

    # CloudFormation
    if (Test-Command "aws") {
        Write-Host "  CloudFormation (aws cli)..."
        $cfnRaw = Get-AwsHelp "cloudformation" | Remove-Formatting
        $cfnCmds = Get-Subcommands $cfnRaw

        $out += "## CloudFormation (CFN)`n`n"
        $out += @"
``````bash
# Validate template
aws cloudformation validate-template --template-body file://template.yaml

# Deploy stack
aws cloudformation deploy \
  --template-file template.yaml \
  --stack-name my-stack \
  --capabilities CAPABILITY_IAM

# Describe stack events (troubleshoot failures)
aws cloudformation describe-stack-events --stack-name my-stack

# Delete stack
aws cloudformation delete-stack --stack-name my-stack
``````

"@
        if ($cfnCmds) {
            $out += "**All CFN commands:** $($cfnCmds -join ', ')`n`n"
        }
    }

    # CDK
    if (Test-Command "cdk") {
        Write-Host "  CDK..."
        $cdkVersion = & cdk --version 2>&1 | Select-Object -First 1
        $cdkHelp = & cdk --help 2>&1 | Select-Object -First 30 | Out-String

        $out += "## AWS CDK`n"
        $out += "<!-- Source: cdk $cdkVersion -->`n`n"
        $out += "``````bash`n$cdkHelp``````n`n"
    } else {
        $out += @"
## AWS CDK

``````bash
# Install: npm install -g aws-cdk
cdk init app --language typescript
cdk synth          # Generate CloudFormation template
cdk diff           # Preview changes
cdk deploy         # Deploy stack
cdk destroy        # Tear down stack
``````

"@
    }

    # SAM
    if (Test-Command "sam") {
        Write-Host "  SAM..."
        $samVersion = & sam --version 2>&1 | Select-Object -First 1
        $samHelp = & sam --help 2>&1 | Select-Object -First 30 | Out-String

        $out += "## AWS SAM`n"
        $out += "<!-- Source: $samVersion -->`n`n"
        $out += "``````bash`n$samHelp``````n`n"
    } else {
        $out += @"
## AWS SAM

``````bash
# Install: pip install aws-sam-cli
sam init                          # Scaffold new project
sam build                         # Build artifacts
sam local invoke MyFunction       # Test locally
sam local start-api               # Local API Gateway
sam deploy --guided               # Deploy to AWS
sam logs -n MyFunction --tail     # Tail function logs
``````

"@
    }

    # Terraform
    if (Test-Command "terraform") {
        Write-Host "  Terraform..."
        $tfVersion = & terraform version 2>&1 | Select-Object -First 1
        $tfHelp = & terraform --help 2>&1 | Select-Object -First 30 | Out-String

        $out += "## Terraform`n"
        $out += "<!-- Source: $tfVersion -->`n`n"
        $out += "``````bash`n$tfHelp``````n`n"
    } else {
        $out += @"
## Terraform

``````bash
# Install: https://developer.hashicorp.com/terraform/install
terraform init       # Initialize provider plugins
terraform plan       # Preview changes
terraform apply      # Apply changes
terraform destroy    # Tear down infrastructure
terraform fmt        # Format HCL files
terraform validate   # Validate configuration
``````

"@
    }

    $out
}

# --- Generate troubleshooting.md ---

function New-Troubleshooting {
    $out = "# AWS Troubleshooting Guide`n"
    $out += "<!-- Generated by bin/gen-aws-docs.ps1 -->`n`n"

    $out += "## Authentication`n`n"

    if (Test-Command "aws") {
        Write-Host "  sts (auth diagnostics)..."
        $stsRaw = Get-AwsHelp "sts" | Remove-Formatting
        $stsCmds = Get-Subcommands $stsRaw

        $out += @"
``````bash
# Verify current identity
aws sts get-caller-identity

# Check configured profiles
aws configure list
aws configure list-profiles

# Assume a role
aws sts assume-role --role-arn arn:aws:iam::ACCOUNT:role/ROLE --role-session-name mysession

# Decode authorization failure message
aws sts decode-authorization-message --encoded-message <message>
``````

"@
        if ($stsCmds) {
            $out += "**STS commands:** $($stsCmds -join ', ')`n`n"
        }
    }

    $out += @"
### Common auth errors

| Error | Cause | Fix |
|-------|-------|-----|
| ``ExpiredTokenException`` | Session/temp credentials expired | Run ``aws sso login`` or refresh credentials |
| ``InvalidClientTokenId`` | Access key ID is wrong or inactive | Check ``aws configure list``, verify key in IAM console |
| ``SignatureDoesNotMatch`` | Secret key mismatch or clock skew | Re-run ``aws configure``, check system clock |
| ``AccessDenied`` | IAM policy doesn't allow action | Check policy with ``aws iam simulate-principal-policy`` |
| ``UnauthorizedAccess`` | No permission / SCP blocking | Check SCPs, permission boundaries, resource policies |

## Debugging

``````bash
# Enable CLI debug output
aws s3 ls --debug 2>&1 | Select-Object -First 50

# Check CLI configuration resolution
aws configure list

# Verify endpoint resolution (PowerShell)
aws s3 ls --debug 2>&1 | Select-String "endpoint_url"
``````

### Debug environment variables

``````powershell
`$env:AWS_DEBUG = "true"              # Enable SDK-level debug
`$env:AWS_CA_BUNDLE = "C:\path\cert"  # Custom CA (corp proxies)
`$env:AWS_MAX_ATTEMPTS = "5"          # Retry count
`$env:AWS_RETRY_MODE = "adaptive"     # Retry strategy
`$env:AWS_DEFAULT_OUTPUT = "json"     # Output format: json|text|table
``````

"@

    if (Test-Command "aws") {
        Write-Host "  CloudWatch Logs (debugging)..."
        $out += @"
### CloudWatch Logs (log tailing)

``````bash
# List log groups
aws logs describe-log-groups --query 'logGroups[].logGroupName' --output text

# Tail logs (live)
aws logs tail /aws/lambda/my-function --follow

# Filter log events
aws logs filter-log-events `
  --log-group-name /aws/lambda/my-function `
  --filter-pattern "ERROR" `
  --start-time (Get-Date).AddHours(-1).ToUnixTimeMilliseconds()
``````

"@
    }

    $out += @"
### Common operational errors

| Error | Service | Fix |
|-------|---------|-----|
| ``ThrottlingException`` | Any | Implement exponential backoff, request limit increase |
| ``ResourceNotFoundException`` | Any | Verify resource name/ARN and region (``--region``) |
| ``BucketAlreadyExists`` | S3 | S3 bucket names are globally unique - pick another |
| ``InstanceLimitExceeded`` | EC2 | Request service quota increase via Service Quotas |
| ``TooManyRequestsException`` | Lambda | Increase reserved concurrency or request limit bump |
"@

    $out
}

# --- Main ---

function Main {
    if ($Help) {
        Write-Host "Usage: gen-aws-docs.ps1 [-Install|-y]"
        Write-Host ""
        Write-Host "Generate AWS reference docs from live CLI help output."
        Write-Host ""
        Write-Host "Options:"
        Write-Host "  -Install, -y    Auto-install AWS CLI if not found"
        Write-Host ""
        Write-Host "Environment:"
        Write-Host "  SWEBASH_AWS_INSTALL=yes       Same as -Install flag"
        Write-Host "  SWEBASH_AWS_DOCS_DIR          Output directory (default: ~/.config/swebash/docs/aws)"
        Write-Host ""
        Write-Host "  AWS CLI installer URL (for mirrors/proxies/specific versions):"
        Write-Host "  SWEBASH_AWS_CLI_URL_WINDOWS   Windows .msi URL"
        return
    }

    $autoInstall = $Install -or $Yes -or ($env:SWEBASH_AWS_INSTALL -eq "yes")

    Write-Host "==> Generating AWS docs to $OutputDir"

    if (-not (Test-Command "aws")) {
        Write-Host ""
        Write-Host "AWS CLI not found."

        if ($autoInstall) {
            Write-Host "Auto-installing AWS CLI (-Install flag)..."
            if (-not (Install-AwsCli -NonInteractive)) {
                Write-Host ""
                Write-Host "Cannot generate docs without AWS CLI."
                Write-Host "Install manually: https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
                exit 1
            }
        } else {
            Write-Host "Install AWS CLI first, or run with -Install flag for auto-install."
            Write-Host "  .\sbh gen-aws-docs -Install"
            Write-Host "  https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
            exit 1
        }
    }

    # Create output directory
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

    Write-Host "==> [1/3] services_reference.md"
    $content1 = New-ServicesReference
    $content1 | Out-File -FilePath "$OutputDir\services_reference.md" -Encoding utf8
    $size1 = (Get-Item "$OutputDir\services_reference.md").Length
    Write-Host "    wrote $size1 bytes"

    Write-Host "==> [2/3] iac_patterns.md"
    $content2 = New-IacPatterns
    $content2 | Out-File -FilePath "$OutputDir\iac_patterns.md" -Encoding utf8
    $size2 = (Get-Item "$OutputDir\iac_patterns.md").Length
    Write-Host "    wrote $size2 bytes"

    Write-Host "==> [3/3] troubleshooting.md"
    $content3 = New-Troubleshooting
    $content3 | Out-File -FilePath "$OutputDir\troubleshooting.md" -Encoding utf8
    $size3 = (Get-Item "$OutputDir\troubleshooting.md").Length
    Write-Host "    wrote $size3 bytes"

    $total = $size1 + $size2 + $size3
    Write-Host ""
    Write-Host "==> Done: 3 files, $total bytes total"
    Write-Host "    $OutputDir\services_reference.md  ($size1 bytes)"
    Write-Host "    $OutputDir\iac_patterns.md        ($size2 bytes)"
    Write-Host "    $OutputDir\troubleshooting.md     ($size3 bytes)"

    if ($total -gt 48000) {
        Write-Host ""
        Write-Host "WARNING: Total output ($total bytes) exceeds 48k char target."
    }
}

# Only run Main when executed directly, not when dot-sourced
if ($MyInvocation.InvocationName -ne '.') {
    Main
}
