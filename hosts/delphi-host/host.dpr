{ Mathless Delphi host — the OTHER half of D14, staged and waiting for a compiler.

  STATUS: THIS FILE HAS NEVER BEEN COMPILED. There is no `dcc64` on the development
  machine (measured: absent from PATH, disk and registry), so every line below is a
  DRAFT. It is written now so that the day a compiler arrives, verification is one
  command away instead of one session away — that was the request that created it.
  Do not cite it as evidence of anything until `MATHLESS_GATE_DELPHI=require` has
  passed on a real machine.

  WHAT IT IS MEANT TO PROVE, once it compiles and runs:
    - the GENERATED `.pas` units are valid Object Pascal and their declarations are
      correct, which is the half of D14 that `hosts/c-host` cannot speak to;
    - a Delphi host reads the same values across the same C ABI as the C host does;
    - the load-time gate (abi version + interface fingerprint) works from Delphi too.

  HOW IT DIFFERS FROM THE C HOST, ON PURPOSE. `hosts/c-host` resolves every symbol
  with LoadLibrary/GetProcAddress. The generated `.pas` instead declares
  `external ML_MODULE`, which Delphi binds when the PROGRAM loads. So this host
  cannot decline to start when a module is missing — the loader refuses first, and
  the process never reaches `begin`. That is a real difference in the binding story
  between the two official hosts, and it is the reason the fingerprint check below
  is written as "refuse to USE" rather than "refuse to load".

  BUILD (once dcc64 exists), from a directory holding the generated artifacts:
      dcc64 -U<artifact_dir> host.dpr
      host.exe <expected_abi_version>
}
program host;

{$APPTYPE CONSOLE}

uses
  SysUtils,
  discount,
  safe_div,
  carrier;

var
  Failures: Integer = 0;

procedure Check(Ok: Boolean; const What: string);
begin
  if Ok then
    Writeln('  ok   ', What)
  else
  begin
    Writeln('  FAIL ', What);
    Inc(Failures);
  end;
end;

{ The load-time gate, in the shape SPEC-iface-hash section 2.6 requires of a host.
  Unlike the C host this runs AFTER the loader has already bound the imports, so it
  gates USE rather than loading — see the header note. }
function GateOk(ExpectedAbi: LongWord): Boolean;
begin
  Result := True;
  if ml_module_abi_version <> ExpectedAbi then
  begin
    Writeln('  refuse: module abi ', ml_module_abi_version, ', host built for ', ExpectedAbi);
    Result := False;
  end;
  if ml_iface_hash <> ML_DISCOUNT_IFACE_HASH then
  begin
    Writeln('  refuse: interface fingerprint differs from the one this unit pins');
    Result := False;
  end;
end;

var
  ExpectedAbi: LongWord;
  Status: Integer;
  OutValue: Double;
  Untouched: Double;
  Buf: array[0..63] of Byte;
  Needed: Integer;
  Tier: Integer;
  I: Integer;
  Canary: Boolean;
begin
  if ParamCount < 1 then
  begin
    Writeln('usage: host <expected_abi_version>');
    Halt(2);
  end;
  ExpectedAbi := StrToInt(ParamStr(1));

  if not GateOk(ExpectedAbi) then
  begin
    Writeln('GATE_DELPHI_REFUSED');
    Halt(1);
  end;
  Check(True, 'discount.dll passed the abi + interface gate');

  { Scalars. `vip` is a 1-byte Boolean on both sides — the generated unit says so, and
    using LongBool here would read three bytes of noise. }
  Check(mlx_discount(100.0, True) = 90.0, 'mlx_discount(100, true) = 90');
  Check(mlx_discount(100.0, False) = 100.0, 'mlx_discount(100, false) = 100');

  { D17: status plus an out-param, and the out-param is NOT written when the call fails. }
  OutValue := -1.0;
  Status := mlx_safe_div(10.0, 2.0, OutValue);
  Check(Status = 0, 'safe_div(10, 2) status = 0');
  Check(OutValue = 5.0, 'safe_div(10, 2) writes 5 through the out-param');

  Untouched := 12345.0;
  Status := mlx_safe_div(1.0, 0.0, Untouched);
  Check(Status = ML_ERR_DIV_BY_ZERO, 'safe_div(1, 0) status = ML_ERR_DIV_BY_ZERO');
  Check(Untouched = 12345.0, 'a failed call leaves the out-param untouched');

  { Q12: the caller owns the buffer. PAnsiChar, never UnicodeString — passing a
    UnicodeString here compiles, does not crash, and matches no code at all, which is
    the silent wrong answer HOST_ABI.md records and the module cannot detect. }
  FillChar(Buf, SizeOf(Buf), $AA);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar('UPSN'), @Buf[0], SizeOf(Buf), Needed);
  Check(Status = 0, 'carrier_name(UPSN) succeeds');
  Check(PAnsiChar(@Buf[0]) = 'UPS Ground', 'carrier_name(UPSN) = "UPS Ground"');

  { Truncation is a FAILURE and writes nothing. One byte short of "UPS Ground" + NUL. }
  FillChar(Buf, SizeOf(Buf), $AA);
  Needed := -1;
  Status := mlx_carrier_name(PAnsiChar('UPSN'), @Buf[0], 10, Needed);
  Check(Status < 0, 'one byte short is a failure, not a short success');
  Check(Needed = 11, 'needed is exact on the failure path');
  Canary := True;
  for I := 0 to SizeOf(Buf) - 1 do
    if Buf[I] <> $AA then
    begin
      Canary := False;
      Break;
    end;
  Check(Canary, 'not one byte of the buffer was written');

  { A declared out plus the buffer triple, in DP-O1 order: declared outs first. }
  FillChar(Buf, SizeOf(Buf), 0);
  Tier := -1;
  Needed := -1;
  Status := mlx_carrier_label(PAnsiChar('UPSN'), Tier, @Buf[0], SizeOf(Buf), Needed);
  Check(Status = 0, 'carrier_label(UPSN) succeeds');
  Check(Tier = 1, 'the declared out comes before the buffer triple');

  if Failures = 0 then
  begin
    Writeln('GATE_DELPHI_OK');
    Halt(0);
  end;
  Writeln('FAILURES: ', Failures);
  Halt(1);
end.
