param(
      [string]$ServerIP = "localhost",

      [int]$Port = 11321,

      [Parameter(Mandatory=$true)]
      [ValidateSet(1,2)]
      [int]$Type,

      [Parameter(Mandatory=$true)]
      [string]$IDM
  )

  # Remove 0x prefix if present and ensure 16 hex characters (8 bytes)
  $cleanHex = $IDM -replace '^0x', ''
  $cleanHex = $cleanHex.PadLeft(16, '0')

  # Convert hex string to bytes (big-endian)
  $intBytes = [byte[]]::new(8)
  for ($i = 0; $i -lt 8; $i++) {
      $intBytes[$i] = [Convert]::ToByte($cleanHex.Substring($i*2, 2), 16)
  }

  # Create packet: type byte + 8 integer bytes
  $packet = [byte[]]::new(9)
  $packet[0] = [byte]$Type
  [Array]::Copy($intBytes, 0, $packet, 1, 8)

  # Send TCP packet
  try {
      $client = New-Object System.Net.Sockets.TcpClient
      $client.Connect($ServerIP, $Port)
      $stream = $client.GetStream()
      $stream.Write($packet, 0, $packet.Length)
      Write-Host "Packet sent successfully"
      $stream.Close()
      $client.Close()
  } catch {
      Write-Error "Failed to send packet: $_"
  }