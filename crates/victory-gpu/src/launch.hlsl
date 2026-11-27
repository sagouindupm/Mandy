// The launch of one mesh draw by the card (`launch.rs`): an amplification
// shader of ours in front of every translated mesh shader. It reads the
// title's record where the title's passes wrote it, bounds it as the clamps
// of `indirect.rs` bound a mesh record (`indirect::batch` above all, which
// clamps the title's single records), charges the list's budget the same
// way, and launches the mesh groups. Nothing runs between the draws: no
// pass of ours, no wait for every earlier shader write.
//
// Compiled by the build (`build.rs`) with the Windows SDK's dxc, which signs
// it with its dxil.dll; `launch::tests` checks the container and
// `pipeline::host::tests::mesh_records_are_launched_by_the_amplification_stage`
// runs it on the card.

// The payload every translated mesh shader declares and never reads
// (`victory_shader::emit::MESH_PAYLOAD_BYTES`, 4 bytes).
struct Payload {
    uint unused;
};
groupshared Payload payload;

// `direct`: the group count of a mesh dispatch the CPU knows (the title's
// DispatchMesh, already bounded there), or, for a record the card reads at
// `record`, RECORD_REST or RECORD_EXACT: what a record that does not fit
// the budget whole does (`indirect::rest`, `VICTORY_INDIRECT_REST`).
cbuffer Launch : register(b0, space9) {
    uint direct;
};
// The record: three words, x, y, z (the console's mesh dispatches are
// one-dimensional).
ByteAddressBuffer record : register(t0, space9);
// The list's work so far in its first word; then, as the clamps leave
// them for the frame line: the largest group count a record asked for,
// the groups asked in all, the records that ran empty for want of budget,
// the largest cost of one (from 2^22).
RWByteAddressBuffer budget : register(u0, space9);

static const uint BUDGET = 1u << 27;
// Takes the rest of the budget: the records after it run empty too.
static const uint RECORD_REST = 0xffffffffu;
// Takes nothing: the next record that fits runs.
static const uint RECORD_EXACT = 0xfffffffeu;

[numthreads(1, 1, 1)]
void main() {
    uint3 g = uint3(0, 0, 0);
    if (direct < RECORD_EXACT) {
        // Known on the CPU, as before: no budget, the host's limit only.
        g = uint3(min(direct, 65535u), 1u, 1u);
    } else {
        bool rest = direct == RECORD_REST;
        g = record.Load3(0);
        // A record over the most a dispatch runs is a slot the title has
        // not filled (the title leaves all ones there): empty, and not
        // counted below either.
        if (g.x > 65535u) {
            g = uint3(0, 0, 0);
        }
        uint asked = g.x;
        g = uint3(g.x, min(g.y, 1u), min(g.z, 1u));
        uint cost = g.x * g.y * g.z * 128u;
        bool fit = true;
        uint told;
        if (cost > 0) {
            uint before;
            budget.InterlockedOr(0, 0, before);
            [allow_uav_condition]
            while (before < BUDGET) {
                uint next = (BUDGET - before < cost) ? (rest ? BUDGET : before) : (before + cost);
                if (next == before) {
                    break;
                }
                uint prev;
                budget.InterlockedCompareExchange(0, before, next, prev);
                if (prev == before) {
                    break;
                }
                before = prev;
            }
            fit = before < BUDGET && BUDGET - before >= cost;
            if (!fit) {
                budget.InterlockedAdd(12, 1u, told);
            }
            if (cost >= (1u << 22)) {
                budget.InterlockedMax(16, cost, told);
            }
        }
        if (!fit) {
            g = uint3(0, 0, 0);
        }
        if (asked > 0) {
            budget.InterlockedMax(4, asked, told);
            budget.InterlockedAdd(8, asked, told);
        }
    }
    payload.unused = 0;
    DispatchMesh(g.x, g.y, g.z, payload);
}
