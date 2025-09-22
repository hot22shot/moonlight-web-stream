# Resolving paths

$metadataJson = cargo metadata --format-version 1 --no-deps
$metadata = $metadataJson | ConvertFrom-Json
$targetDir = $metadata.target_directory

New-Item -ItemType Directory "./finalOutput" -Force
$outputDir = Resolve-Path "./finalOutput"

$moonlightRoot = Resolve-Path "."
$moonlightFrontend = Join-Path -Path $moonlightRoot -ChildPath "/moonlight-web/web-server"

if(!$moonlightRoot -or !$moonlightFrontend) {
    echo "No root directory found!"
    exit 0
}

echo "Target directory at $targetDir"
echo "Putting final output into $outputDir"
echo "Moonlight Root Directory $moonlightRoot"

$targets = @(
    "x86_64-pc-windows-gnu"
    "x86_64-unknown-linux-gnu"
    "arm-unknown-linux-gnueabihf"
    "aarch64-unknown-linux-gnu"
)

Remove-Item -Path "$outputDir/*" -Recurse -Force

echo "------------- Starting Build for Frontend -------------"
Set-Location $moonlightFrontend

New-Item -ItemType Directory "$outputDir/static" -Force | Out-Null

Remove-Item -Path "$moonlightFrontend/dist" -Recurse -Force
npm run build

Copy-Item -Path "$moonlightFrontend/dist/*" -Destination "$outputDir/static" -Recurse -Force
echo "------------- Finished Build for Frontend -------------"

Set-Location $moonlightRoot

foreach($target in $targets) {
    echo "------------- Starting Build for $target -------------"
    $messages = cross build --release --target $target --message-format=json | ForEach-Object { $_ | ConvertFrom-Json }
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    echo "------------- Finished Build for $target -------------"

    $artifact = $messages | Where-Object { $_.reason -eq "compiler-artifact" -and $_.executable }
    $binaryPaths = $artifact | ForEach-Object { Join-Path -Path $targetDir -ChildPath ($_.executable.Substring("/target".length)) }

    $binaryPaths | ForEach-Object { Write-Host "Binary: $_" }

    echo "------------- Starting Zipping for $target -------------"
    $itemsToZip = @($binaryPaths) + "$outputDir/static"
    $zipDestination = "$outputDir/moonlight-web-$target.zip"

    Compress-Archive -Path $itemsToZip -DestinationPath $zipDestination -Force

    echo "Created Zip file at $zipDestination"
    echo "------------- Finished Zipping for $target -------------"
}

echo "Finished!"