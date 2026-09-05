pub fn list() -> Result<Vec<String>, String> {
    let midi = midir::MidiOutput::new("Phasecraft").map_err(|e| e.to_string())?;
    midi.ports()
        .iter()
        .map(|p| midi.port_name(p).map_err(|e| e.to_string()))
        .collect()
}
pub fn open_output(
    port: Option<String>,
    virtual_port: bool,
) -> Result<midir::MidiOutputConnection, String> {
    let midi = midir::MidiOutput::new("Phasecraft").map_err(|e| e.to_string())?;
    if virtual_port {
        #[cfg(unix)]
        {
            use midir::os::unix::VirtualOutput;
            return midi.create_virtual("Phasecraft").map_err(|e| e.to_string());
        }
        #[cfg(not(unix))]
        {
            return Err("Virtual source creation is unavailable on this platform; use --port with an existing MIDI loopback destination".into());
        }
    }
    let name = port.ok_or("Choose --port NAME, --virtual-port, or --dry-run; use `phasecraft ports` to list destinations")?;
    let ports = midi.ports();
    let selected = if let Ok(index) = name.parse::<usize>() {
        ports.get(index)
    } else {
        ports
            .iter()
            .find(|p| midi.port_name(p).ok().as_deref() == Some(name.as_str()))
    };
    let selected = selected
        .ok_or_else(|| format!("MIDI output {name:?} not found; run `phasecraft ports`"))?;
    midi.connect(selected, "Phasecraft")
        .map_err(|e| e.to_string())
}
