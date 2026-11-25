//! The PC container around emitted DXIL: feature flags (`SFI0`), the
//! signatures (`ISG1`, `OSG1`), pipeline state validation (`PSV0`) and the
//! program (`DXIL`), laid out as validator version 1.6 lays them out, plus
//! the container hash that D3D12 checks.

/// One signature element: one row (D3D12 links stages by semantic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SigElement {
    pub name: String,
    pub index: u32,
    /// DXIL semantic kind (0 arbitrary, 1 vertex id, 3 position, 16 target...).
    pub kind: u8,
    /// DXIL component type (4 i32, 5 u32, 9 f32, 1 i1).
    pub comp: u8,
    /// DXIL interpolation mode (0 for none).
    pub interp: u8,
    /// Start row, if the element has a register.
    pub row: Option<u32>,
    pub col: u8,
    pub cols: u8,
    /// Components read (inputs) or written (outputs), relative to `col`.
    pub usage: u8,
    /// 16-bit on the console (declared 32-bit here).
    pub half: bool,
}

/// Gives the elements sharing a signature row one interpolation mode, as
/// DXIL requires ("interpolation mode that differs from another element
/// packed into the same row"). The console packed rows whose elements the
/// source declared alike; here each element's mode is chosen on its own
/// (by type, or by how the program reads it), so an unread element can
/// keep a default its row-mates do not share. The mode of the elements
/// that are used wins when they agree; with none used, or on the output
/// side when they disagree, a constant member makes the row constant (a
/// float may be declared constant; an integer may not be interpolated).
/// Rows whose used inputs disagree are left for the validator to name.
pub(crate) fn unify_row_modes(elements: &mut [SigElement], inputs: bool) {
    let rows: std::collections::BTreeSet<u32> = elements
        .iter()
        .filter(|e| e.interp != 0)
        .filter_map(|e| e.row)
        .collect();
    for row in rows {
        let members = |e: &SigElement| e.interp != 0 && e.row == Some(row);
        let mut modes = elements.iter().filter(|e| members(e)).map(|e| e.interp);
        let first = modes.next().unwrap_or(0);
        if modes.all(|m| m == first) {
            continue;
        }
        let used: std::collections::BTreeSet<u8> = elements
            .iter()
            .filter(|e| members(e) && e.usage != 0)
            .map(|e| e.interp)
            .collect();
        let constant = elements.iter().any(|e| members(e) && e.interp == 1);
        let target = match used.len() {
            1 => used.first().copied(),
            0 if constant => Some(1),
            0 => None,
            _ if !inputs && constant => Some(1),
            _ => None,
        };
        let Some(target) = target else { continue };
        for e in elements.iter_mut().filter(|e| members(e)) {
            e.interp = target;
        }
    }
}

impl SigElement {
    fn system_value(&self) -> u32 {
        match self.kind {
            1 => 6,  // vertex id
            2 => 8,  // instance id
            3 => 1,  // position
            4 => 4,  // render target array index
            5 => 5,  // viewport array index
            6 => 2,  // clip distance
            7 => 3,  // cull distance
            10 => 7, // primitive id
            12 => 10,
            13 => 9, // front face
            14 => 66,
            15 => 70,
            16 => 64, // target
            17 => 65, // depth
            18 => 68,
            19 => 67,
            20 => 69,
            28 => 23, // barycentrics
            30 => 25, // cull primitive
            _ => 0,
        }
    }

    fn sig_comp(&self) -> u32 {
        match self.comp {
            1 | 5 => 1, // bool and u32
            4 => 2,
            9 => 3,
            _ => 0,
        }
    }

    fn mask(&self) -> u8 {
        ((1u8 << self.cols) - 1) << self.col
    }
}

/// Read the mesh shader's declared interpolation instead of deriving it from the value type.
/// The vertex outputs follow the input records in PSV0. OSG1 can list the same elements
/// in a different order, so apply the modes only after matching the two descriptions.
pub(super) fn mesh_output_modes(psv: &[u8], osg: &[u8], outputs: &[SigElement]) -> Option<Vec<u8>> {
    use crate::dxbc::u32_at;

    fn word(bytes: &[u8]) -> Option<usize> {
        Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize)
    }

    fn table(bytes: &[u8], stride: usize) -> Option<(&[u8], &[u8])> {
        bytes
            .get(4..)?
            .split_at_checked(word(bytes)?.checked_mul(stride)?)
    }

    let (info, mut tables) = table(psv, 1)?;
    if info.len() < 36 || info[24] != 13 {
        return None;
    }
    let [inputs, vertices, primitives] = [info[28], info[29], info[30]].map(usize::from);
    if vertices == 0 {
        return None;
    }
    let resources = word(tables)?;
    tables = tables.get(4..)?;
    if resources != 0 {
        let stride = word(tables)?;
        if stride < 16 {
            return None;
        }
        tables = tables.get(4..)?.get(resources.checked_mul(stride)?..)?;
    }
    let (names, tables) = table(tables, 1)?;
    if names.len() % 4 != 0 {
        return None;
    }
    let (indices, tables) = table(tables, 4)?;
    let stride = word(tables)?;
    if stride < 16 {
        return None;
    }
    let records = tables
        .get(4..)?
        .get(..(inputs + vertices + primitives).checked_mul(stride)?)?;
    let vertices = &records[inputs * stride..(inputs + vertices) * stride];
    let osg_rows = osg.get(u32_at(osg, 4).ok()? as usize..)?;
    let mut modes: Vec<_> = outputs.iter().map(|e| e.interp).collect();
    for (i, output) in outputs.iter().enumerate() {
        let Some(row) = output.row.filter(|_| output.kind == 0 && output.comp == 9) else {
            continue;
        };
        let raw = osg_rows.get(i * 32..i * 32 + 32)?;
        let mask = raw[24];
        if mask == 0 || mask & 0xf0 != 0 || mask != output.mask() {
            return None;
        }
        let raw_comp = u32_at(raw, 16).ok()?;
        let mut matched = None;
        for record in vertices.chunks_exact(stride) {
            let cols = record[10];
            if record[11] != 0
                || u32::from(record[12]) != raw_comp
                || cols & 0x40 == 0
                || u32::from(record[9]) != row
                || cols & 0x3f != output.cols | (output.col << 4)
            {
                continue;
            }
            // A multirow element has one index for each row. This path only
            // uses one-row elements with an exact OSG1 counterpart.
            if record[8] != 1 || cols & 0x80 != 0 || record[14] & 0x30 != 0 {
                return None;
            }
            let index = word(&record[4..])?.checked_mul(4)?;
            let name = names.get(word(record)?..)?;
            let name = name.get(..name.iter().position(|b| *b == 0)?)?;
            if u32_at(indices.get(index..)?, 0).ok()? != output.index
                || name != output.name.as_bytes()
            {
                continue;
            }
            let mode = record[13];
            if !(1..=7).contains(&mode) || matched.replace(mode).is_some() {
                return None;
            }
        }
        modes[i] = matched?;
    }
    // The host requires one defined mode for every member of a packed row.
    for (i, output) in outputs.iter().enumerate() {
        if modes[i] != output.interp
            && outputs
                .iter()
                .zip(&modes)
                .any(|(other, &mode)| other.row == output.row && mode != 0 && mode != modes[i])
        {
            return None;
        }
    }
    Some(modes)
}

/// `ISG1`/`OSG1`: 32-byte rows, then the names.
pub(crate) fn signature(elements: &[SigElement], input: bool) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend((elements.len() as u32).to_le_bytes());
    out.extend(8u32.to_le_bytes());
    // Validator 1.6 writes one name per element in element order, repeats
    // included, except that system values share theirs (it keys names by
    // the semantic's string, which is one static string per system value);
    // the rows in element order too, with only the unallocated ones moved
    // last. Sorting the rows by register instead fails ("Container part
    // ... does not match expected") wherever the console lists an element
    // before one in an earlier column of its row (TEXCOORD16 before
    // TANGENT0, both in row 10: PS_MirrorReflectionDeferredPedTexturedNC).
    let mut offsets: Vec<u32> = Vec::new();
    let mut next = 8 + 32 * elements.len() as u32;
    let mut names = Vec::new();
    for (i, e) in elements.iter().enumerate() {
        let shared = (e.kind != 0)
            .then(|| (0..i).find(|&j| elements[j].kind == e.kind))
            .flatten();
        match shared {
            Some(j) => offsets.push(offsets[j]),
            None => {
                offsets.push(next);
                next += e.name.len() as u32 + 1;
                names.push(&e.name);
            }
        }
    }
    let mut order: Vec<usize> = (0..elements.len()).collect();
    order.sort_by_key(|&i| elements[i].row.is_none());
    for i in order {
        let (e, offset) = (&elements[i], offsets[i]);
        let used = e.usage << e.col;
        let rw = if input { used } else { !used & 0xf };
        for v in [
            0,
            offset,
            e.index,
            e.system_value(),
            e.sig_comp(),
            e.row.unwrap_or(u32::MAX),
        ] {
            out.extend(v.to_le_bytes());
        }
        out.extend([e.mask(), rw, 0, 0]);
        out.extend(0u32.to_le_bytes());
    }
    for name in names {
        out.extend(name.as_bytes());
        out.push(0);
    }
    out
}

/// The most a mesh shader's outputs may take, in bytes (D3D12).
pub(crate) const MESH_OUTPUT_LIMIT: u64 = 32 * 1024;

/// A mesh shader's output size as the DXIL validator measures it against
/// [`MESH_OUTPUT_LIMIT`]: 4 bytes per component of every vertex element
/// and primitive element, times the maxima rounded up to multiples of 32.
pub(crate) fn mesh_output_bytes(
    vertex: &[SigElement],
    primitive: &[SigElement],
    max_vertices: u32,
    max_primitives: u32,
) -> u64 {
    let scalars = |els: &[SigElement]| els.iter().map(|e| u64::from(e.cols)).sum::<u64>();
    let align = |n: u32| u64::from(n).next_multiple_of(32);
    4 * (scalars(vertex) * align(max_vertices) + scalars(primitive) * align(max_primitives))
}

/// A resource bound by register, as `PSV0` lists it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Binding {
    /// PSV resource type: 1 sampler, 2 CBV, 4 raw SRV, 7 raw UAV.
    pub kind: u32,
    pub space: u32,
    pub register: u32,
    /// DXIL resource kind (13 cbuffer, 14 sampler).
    pub dxil_kind: u32,
}

pub(crate) struct Psv<'a> {
    /// PSV shader kind: 0 pixel, 1 vertex, 2 geometry, 5 compute.
    pub stage: u8,
    /// Vertex: writes `SV_Position`. Pixel: writes depth.
    pub flag: bool,
    pub inputs: &'a [SigElement],
    pub outputs: &'a [SigElement],
    /// Mesh: per-primitive outputs.
    pub prims: &'a [SigElement],
    /// Mesh: its state and the group-shared bytes it declares.
    pub mesh: Option<(super::shader::Mesh, u32)>,
    /// Geometry (triangles in, triangle strips out on stream 0): its
    /// maximum vertex count. `flag`: writes `SV_Position`.
    pub geometry: Option<u32>,
    pub bindings: &'a [Binding],
    /// Compute: thread-group size and required wave size.
    pub threads: [u32; 3],
    pub wave_size: Option<u32>,
}

/// `PSV0` with runtime info version 2 (validator 1.6).
pub(crate) fn psv(p: &Psv) -> Vec<u8> {
    let vectors = |els: &[SigElement]| {
        els.iter()
            .filter_map(|e| e.row)
            .map(|r| r + 1)
            .max()
            .unwrap_or(0)
    };
    let (in_vectors, out_vectors) = (vectors(p.inputs), vectors(p.outputs));
    let mut out = Vec::new();
    let u32s = |out: &mut Vec<u8>, vs: &[u32]| {
        for v in vs {
            out.extend(v.to_le_bytes());
        }
    };
    u32s(&mut out, &[48]);
    let mut info = [0u8; 48];
    info[0] = u8::from(p.flag);
    if p.stage == 0 {
        // PSInfo.SampleFrequency: an input interpolated per sample, or the
        // sample index.
        info[1] = u8::from(
            p.inputs
                .iter()
                .any(|e| matches!(e.interp, 6 | 7) || e.kind == 12),
        );
    }
    let (min, max) = p.wave_size.map_or((0, u32::MAX), |n| (n, n));
    info[16..20].copy_from_slice(&min.to_le_bytes());
    info[20..24].copy_from_slice(&max.to_le_bytes());
    info[24] = p.stage;
    for (k, n) in p.threads.into_iter().enumerate() {
        info[36 + 4 * k..40 + 4 * k].copy_from_slice(&n.to_le_bytes());
    }
    info[28] = p.inputs.len() as u8;
    info[29] = p.outputs.len() as u8;
    info[31] = in_vectors as u8;
    info[32] = out_vectors as u8;
    if let Some((mesh, shared)) = p.mesh {
        info[0..4].copy_from_slice(&shared.to_le_bytes());
        info[12..14].copy_from_slice(&(mesh.max_vertices as u16).to_le_bytes());
        info[14..16].copy_from_slice(&(mesh.max_primitives as u16).to_le_bytes());
        info[26] = vectors(p.prims) as u8;
        info[27] = mesh.topology;
        info[30] = p.prims.len() as u8;
    }
    if let Some(max_vertices) = p.geometry {
        // Input primitive (3: triangles), output topology (5: triangle
        // strips), stream mask, position written; the vertex maximum.
        info[0..4].copy_from_slice(&3u32.to_le_bytes());
        info[4..8].copy_from_slice(&5u32.to_le_bytes());
        info[8..12].copy_from_slice(&1u32.to_le_bytes());
        info[12] = u8::from(p.flag);
        info[26..28].copy_from_slice(&(max_vertices as u16).to_le_bytes());
    }
    out.extend(info);
    u32s(&mut out, &[p.bindings.len() as u32]);
    if !p.bindings.is_empty() {
        u32s(&mut out, &[24]);
        for b in p.bindings {
            u32s(
                &mut out,
                &[b.kind, b.space, b.register, b.register, b.dxil_kind, 0],
            );
        }
    }
    // Names of arbitrary semantics (system values use ""), then indices.
    let mut strings = vec![0u8];
    let mut indices: Vec<u32> = Vec::new();
    let mut elements = Vec::new();
    for e in p.inputs.iter().chain(p.outputs).chain(p.prims) {
        let name = if e.kind == 0 {
            let needle: Vec<u8> = e.name.bytes().chain([0]).collect();
            match find(&strings, &needle) {
                Some(at) => at,
                None => {
                    strings.extend(&needle);
                    strings.len() - needle.len()
                }
            }
        } else {
            0
        };
        let index = match indices.iter().position(|&i| i == e.index) {
            Some(at) => at,
            None => {
                indices.push(e.index);
                indices.len() - 1
            }
        };
        let (start, alloc) = match e.row {
            Some(r) => (r as u8, 0x40 | (e.col << 4)),
            None => (0, 0),
        };
        elements.push((name as u32, index as u32, start, e.cols | alloc, e));
    }
    strings.resize(strings.len().next_multiple_of(4), 0);
    u32s(&mut out, &[strings.len() as u32]);
    out.extend(&strings);
    u32s(&mut out, &[indices.len() as u32]);
    u32s(&mut out, &indices);
    if !elements.is_empty() {
        u32s(&mut out, &[16]);
        for (name, index, start, cols, e) in elements {
            u32s(&mut out, &[name, index]);
            out.extend([1, start, cols, e.kind, e.sig_comp() as u8, e.interp, 0, 0]);
        }
    }
    // Input-to-output dependencies, as the view-id state declares them:
    // every output scalar on every input scalar.
    if in_vectors > 0 && out_vectors > 0 {
        let scalars = |els: &[SigElement]| {
            els.iter()
                .filter_map(|e| e.row.map(|r| r * 4 + u32::from(e.col + e.cols)))
                .max()
                .unwrap_or(0)
        };
        let (ni, no) = (scalars(p.inputs), scalars(p.outputs));
        let per = (out_vectors * 4).div_ceil(32);
        for i in 0..in_vectors * 4 {
            for k in 0..per {
                let bits = if i < ni {
                    no.saturating_sub(32 * k).min(32)
                } else {
                    0
                };
                let mask = if bits == 32 {
                    u32::MAX
                } else {
                    (1u32 << bits) - 1
                };
                u32s(&mut out, &[mask]);
            }
        }
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The container: parts in order, hashed.
pub(crate) fn build(parts: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
    let table = 0x20 + 4 * parts.len();
    let total = table + parts.iter().map(|(_, d)| 8 + d.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend(b"DXBC");
    out.extend([0; 16]);
    out.extend(1u16.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((parts.len() as u32).to_le_bytes());
    let mut offset = table;
    for (_, d) in parts {
        out.extend((offset as u32).to_le_bytes());
        offset += 8 + d.len();
    }
    for (fourcc, d) in parts {
        out.extend(*fourcc);
        out.extend((d.len() as u32).to_le_bytes());
        out.extend(*d);
    }
    let digest = hash(&out[20..]);
    out[4..20].copy_from_slice(&digest);
    out
}

/// The container hash D3D12 checks: MD5 whose last block carries the bit
/// count first and `2 × bytes + 1` last instead of standard padding.
pub(crate) fn hash(data: &[u8]) -> [u8; 16] {
    let len = data.len();
    let left = len % 64;
    let mut tail = Vec::with_capacity(128);
    if left < 56 {
        tail.extend(((len as u32) << 3).to_le_bytes());
        tail.extend(&data[len - left..]);
        tail.push(0x80);
        tail.resize(60, 0);
    } else {
        tail.extend(&data[len - left..]);
        tail.push(0x80);
        tail.resize(64, 0);
        tail.extend(((len as u32) << 3).to_le_bytes());
        tail.resize(124, 0);
    }
    tail.extend(((len as u32) << 1 | 1).to_le_bytes());
    let mut state = [0x6745_2301u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    for block in data[..len - left]
        .as_chunks::<64>()
        .0
        .iter()
        .chain(tail.as_chunks::<64>().0)
    {
        md5_block(&mut state, block);
    }
    let mut out = [0; 16];
    for (o, s) in out.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        o.copy_from_slice(&s.to_le_bytes());
    }
    out
}

fn md5_block(state: &mut [u32; 4], block: &[u8; 64]) {
    const S: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
    let x: Vec<u32> = block
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect();
    let [mut a, mut b, mut c, mut d] = *state;
    for i in 0..64 {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let k = ((i as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32;
        let rotated = a
            .wrapping_add(f)
            .wrapping_add(k)
            .wrapping_add(x[g])
            .rotate_left(S[(i / 16) * 4 + i % 4]);
        (a, b, c, d) = (d, b.wrapping_add(rotated), b, c);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d]) {
        *s = s.wrapping_add(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn element(
        name: &str,
        index: u32,
        kind: u8,
        interp: u8,
        row: u32,
        col: u8,
        cols: u8,
        usage: u8,
    ) -> SigElement {
        SigElement {
            name: name.into(),
            index,
            kind,
            comp: if kind == 13 { 5 } else { 9 },
            interp,
            row: Some(row),
            col,
            cols,
            usage,
            half: false,
        }
    }

    /// PS_PropFoliageDeferred_Fire_ExportDepth (999de9c4262184ce): an
    /// unread TEXCOORD9 shares row 9 with the constant front face.
    #[test]
    fn an_unread_element_takes_its_row_mates_mode() {
        let mut sig = vec![
            element("TEXCOORD", 0, 0, 2, 1, 0, 4, 0xf),
            element("TEXCOORD", 9, 0, 2, 9, 0, 1, 0),
            element("SV_IsFrontFace", 0, 13, 1, 9, 1, 1, 1),
        ];
        unify_row_modes(&mut sig, true);
        assert_eq!(sig.iter().map(|e| e.interp).collect::<Vec<_>>(), [2, 1, 1]);
    }

    #[test]
    fn read_inputs_that_disagree_keep_their_modes() {
        let mut sig = vec![
            element("TEXCOORD", 0, 0, 2, 3, 0, 3, 0b111),
            element("TEXCOORD", 1, 0, 1, 3, 3, 1, 1),
        ];
        unify_row_modes(&mut sig, true);
        assert_eq!(sig.iter().map(|e| e.interp).collect::<Vec<_>>(), [2, 1]);
    }

    #[test]
    fn an_output_row_with_a_constant_member_is_constant() {
        // VS outputs: COLOR0 (uint, constant) and TEXCOORD22 (float) in row 14.
        let mut sig = vec![
            element("COLOR", 0, 0, 1, 14, 0, 1, 1),
            element("TEXCOORD", 22, 0, 2, 14, 1, 2, 0b11),
        ];
        unify_row_modes(&mut sig, false);
        assert_eq!(sig.iter().map(|e| e.interp).collect::<Vec<_>>(), [1, 1]);
    }

    /// PS_MirrorReflectionDeferredPedTexturedNC (7e80c1635201799c) lists
    /// TEXCOORD16 (row 10, w) before TANGENT0 (row 10, xyz); the part keeps
    /// that order, as the validator's own serialisation does.
    #[test]
    fn signature_rows_keep_element_order() {
        let sig = vec![
            element("TEXCOORD", 16, 0, 2, 10, 3, 1, 1),
            element("TANGENT", 0, 0, 2, 10, 0, 3, 0b111),
        ];
        let part = signature(&sig, true);
        let row = |i: usize| u32::from_le_bytes(part[8 + 32 * i + 8..][..4].try_into().unwrap());
        assert_eq!(
            (row(0), row(1)),
            (16, 0),
            "semantic indices in element order"
        );
    }

    #[test]
    fn mesh_vertex_modes_follow_psv_semantics() {
        let element = |index, col, cols, comp, interp| SigElement {
            name: "TEXCOORD".into(),
            index,
            kind: 0,
            comp,
            interp,
            row: Some(9),
            col,
            cols,
            usage: (1 << cols) - 1,
            half: false,
        };
        let integer = element(8, 0, 3, 5, 1);
        let flat_float = element(11, 3, 1, 9, 1);
        let guest_outputs = [flat_float.clone(), integer.clone()];
        let guest_psv = psv(&Psv {
            stage: 13,
            flag: false,
            inputs: &[],
            outputs: &guest_outputs,
            prims: &[],
            mesh: None,
            geometry: None,
            bindings: &[Binding {
                kind: 2,
                space: 0,
                register: 0,
                dxil_kind: 13,
            }],
            threads: [32, 1, 1],
            wave_size: None,
        });
        let mut outputs = [
            integer,
            SigElement {
                interp: 2,
                ..flat_float
            },
        ];
        let osg = signature(&outputs, false);
        assert_eq!(
            mesh_output_modes(&guest_psv, &osg, &outputs),
            Some(vec![1, 1])
        );
        assert_eq!(
            mesh_output_modes(&guest_psv[..guest_psv.len() - 1], &osg, &outputs),
            None
        );

        outputs[1].index = 12;
        assert_eq!(mesh_output_modes(&guest_psv, &osg, &outputs), None);
        outputs[1].index = 11;
        outputs[0].interp = 2;
        assert_eq!(mesh_output_modes(&guest_psv, &osg, &outputs), None);
    }

    #[test]
    fn md5_core_matches_the_standard() {
        // Standard MD5 of "abc" with standard padding, through md5_block.
        let mut block = b"abc".to_vec();
        block.push(0x80);
        block.resize(56, 0);
        block.extend(24u64.to_le_bytes());
        let mut state = [0x6745_2301u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
        md5_block(&mut state, block.as_slice().try_into().unwrap());
        let hex: String = state
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(hex, "900150983cd24fb0d6963f7d28e17f72");
    }
}
