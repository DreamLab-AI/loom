#!/usr/bin/env python3
"""
Multi-variate model comparison plot with power analysis.
Accounts for 2x RTX 6000 Ada @ 300W TDP each, UK electricity at 24.5p/kWh.
"""
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import matplotlib.gridspec as gridspec
import numpy as np

# ── Model Data (from bench-quality-v2 results) ──

models = {
    "Nemotron 120B\n(IQ4_XS)": {
        "score": 30.0, "total": 33, "pct": 90.9,
        "gen_speed": 58.8, "prompt_speed": 458.2,
        "disk_gb": 61, "vram_gb": 76,
        "gpu_count": 2,  # needs both GPUs
        "categories": {
            "Knowledge": 100, "Math": 80, "Reasoning": 80,
            "Coding": 80, "Instruction": 100, "Creative": 100,
            "Abliteration": 100, "Context": 100,
        },
        "color": "#e74c3c", "marker": "s",
    },
    "Gemma 26B MoE\nBF16 (ablit.)": {
        "score": 30.0, "total": 33, "pct": 90.9,
        "gen_speed": 82.1, "prompt_speed": 925.1,
        "disk_gb": 48, "vram_gb": 48,
        "gpu_count": 2,
        "categories": {
            "Knowledge": 100, "Math": 100, "Reasoning": 80,
            "Coding": 80, "Instruction": 80, "Creative": 100,
            "Abliteration": 100, "Context": 100,
        },
        "color": "#2ecc71", "marker": "D",
    },
    "Gemma 31B\nQ8_0 (orig.)": {
        "score": 30.0, "total": 33, "pct": 90.9,
        "gen_speed": 24.6, "prompt_speed": 592.7,
        "disk_gb": 32, "vram_gb": 33,
        "gpu_count": 2,
        "categories": {
            "Knowledge": 100, "Math": 100, "Reasoning": 80,
            "Coding": 80, "Instruction": 80, "Creative": 100,
            "Abliteration": 100, "Context": 100,
        },
        "color": "#3498db", "marker": "o",
    },
    "Gemma 26B MoE\nAPEX-IQ (ablit.)": {
        "score": 29.5, "total": 33, "pct": 89.4,
        "gen_speed": 152.2, "prompt_speed": 1378.4,
        "disk_gb": 21, "vram_gb": 20,
        "gpu_count": 2,
        "categories": {
            "Knowledge": 100, "Math": 100, "Reasoning": 80,
            "Coding": 80, "Instruction": 80, "Creative": 83.3,
            "Abliteration": 100, "Context": 100,
        },
        "color": "#f39c12", "marker": "^",
    },
    "Gemma 31B\nQ4_K_M (ablit.)": {
        "score": 28.0, "total": 33, "pct": 84.8,
        "gen_speed": 39.9, "prompt_speed": 778.1,
        "disk_gb": 18, "vram_gb": 18,
        "gpu_count": 2,
        "categories": {
            "Knowledge": 100, "Math": 80, "Reasoning": 80,
            "Coding": 80, "Instruction": 60, "Creative": 100,
            "Abliteration": 100, "Context": 100,
        },
        "color": "#9b59b6", "marker": "v",
    },
    "Qwen 122B\n(UD-Q4_K_XL)": {
        "score": 21.0, "total": 33, "pct": 63.6,
        "gen_speed": 70.4, "prompt_speed": 479.0,
        "disk_gb": 72, "vram_gb": 73,
        "gpu_count": 2,
        "categories": {
            "Knowledge": 100, "Math": 60, "Reasoning": 80,
            "Coding": 20, "Instruction": 60, "Creative": 0,
            "Abliteration": 100, "Context": 100,
        },
        "color": "#95a5a6", "marker": "X",
    },
}

# ── Power Calculations ──
# 2x RTX 6000 Ada: 300W TDP each = 600W GPU max
# System (CPU, RAM, etc): ~175W
# Inference typically uses 60-80% of GPU TDP depending on model utilization
# MoE models use less GPU power (sparse computation)

GPU_TDP_W = 300  # per GPU
SYSTEM_OVERHEAD_W = 175
UK_RATE_PER_KWH = 0.245  # GBP
HOURS_PER_DAY = 4
DAYS_PER_MONTH = 30

for name, m in models.items():
    # Estimate GPU utilization based on gen speed relative to memory bandwidth
    # MoE models (26B) use less power than dense models (31B, 120B, 122B)
    is_moe = "MoE" in name or "122B" in name or "120B" in name
    is_small_quant = m["vram_gb"] < 25

    if is_moe and is_small_quant:
        gpu_util = 0.45  # light MoE quant
    elif is_moe:
        gpu_util = 0.55  # MoE BF16 or large
    elif m["vram_gb"] > 60:
        gpu_util = 0.75  # large dense
    else:
        gpu_util = 0.65  # medium dense

    gpu_power = m["gpu_count"] * GPU_TDP_W * gpu_util
    total_power_w = gpu_power + SYSTEM_OVERHEAD_W
    m["power_w"] = total_power_w
    m["power_kwh_day"] = total_power_w * HOURS_PER_DAY / 1000
    m["cost_day_gbp"] = m["power_kwh_day"] * UK_RATE_PER_KWH
    m["cost_month_gbp"] = m["cost_day_gbp"] * DAYS_PER_MONTH
    # Efficiency: tokens per penny
    m["tokens_per_penny"] = m["gen_speed"] / (m["cost_day_gbp"] / (HOURS_PER_DAY * 3600) * 100)
    # Quality-adjusted efficiency: score * speed / power
    m["efficiency"] = (m["pct"] / 100) * m["gen_speed"] / (total_power_w / 100)

# ── Composite "Winner" Score ──
# Weighted: 40% quality, 25% speed, 20% efficiency, 15% VRAM economy
max_speed = max(m["gen_speed"] for m in models.values())
max_eff = max(m["efficiency"] for m in models.values())
max_vram = max(m["vram_gb"] for m in models.values())

for name, m in models.items():
    quality_norm = m["pct"] / 100
    speed_norm = m["gen_speed"] / max_speed
    eff_norm = m["efficiency"] / max_eff
    vram_norm = 1 - (m["vram_gb"] / max_vram)  # lower VRAM = better
    m["composite"] = 0.40 * quality_norm + 0.25 * speed_norm + 0.20 * eff_norm + 0.15 * vram_norm

winner = max(models.items(), key=lambda x: x[1]["composite"])

# ── Create Figure ──
fig = plt.figure(figsize=(22, 16), facecolor='#0d1117')
fig.suptitle('LLM Model Comparison — Quality, Speed, Power & Cost',
             fontsize=20, color='white', fontweight='bold', y=0.98)
fig.text(0.5, 0.955, f'2x RTX 6000 Ada (98GB) | UK Electricity 24.5p/kWh | {HOURS_PER_DAY}h/day',
         ha='center', fontsize=11, color='#8b949e')

gs = gridspec.GridSpec(3, 3, hspace=0.35, wspace=0.3,
                       left=0.06, right=0.96, top=0.92, bottom=0.06)

dark_bg = '#0d1117'
panel_bg = '#161b22'
grid_color = '#21262d'
text_color = '#c9d1d9'
accent = '#58a6ff'

def style_ax(ax, title):
    ax.set_facecolor(panel_bg)
    ax.set_title(title, color='white', fontsize=13, fontweight='bold', pad=10)
    ax.tick_params(colors=text_color, labelsize=9)
    ax.spines['bottom'].set_color(grid_color)
    ax.spines['left'].set_color(grid_color)
    ax.spines['top'].set_visible(False)
    ax.spines['right'].set_visible(False)
    ax.grid(True, alpha=0.15, color=grid_color)

names = list(models.keys())
short_names = [n.split('\n')[0] for n in names]
colors = [models[n]["color"] for n in names]
markers = [models[n]["marker"] for n in names]

# ── Panel 1: Speed vs Quality (bubble = VRAM) ──
ax1 = fig.add_subplot(gs[0, 0])
style_ax(ax1, 'Speed vs Quality (bubble = VRAM)')
for i, (name, m) in enumerate(models.items()):
    size = m["vram_gb"] * 8
    ax1.scatter(m["gen_speed"], m["pct"], s=size, c=m["color"],
                marker=m["marker"], alpha=0.85, edgecolors='white', linewidth=1.5,
                zorder=5, label=short_names[i])
    if name == winner[0]:
        ax1.scatter(m["gen_speed"], m["pct"], s=size+200, facecolors='none',
                    edgecolors='#ffd700', linewidth=3, zorder=4)
ax1.set_xlabel('Generation Speed (t/s)', color=text_color, fontsize=10)
ax1.set_ylabel('Quality Score (%)', color=text_color, fontsize=10)
ax1.legend(fontsize=7, loc='lower right', facecolor=panel_bg, edgecolor=grid_color,
           labelcolor=text_color)

# ── Panel 2: Power Draw & Daily Cost ──
ax2 = fig.add_subplot(gs[0, 1])
style_ax(ax2, 'Estimated Power Draw & Daily Cost')
power_vals = [models[n]["power_w"] for n in names]
cost_vals = [models[n]["cost_day_gbp"] for n in names]
x = np.arange(len(names))
bars = ax2.barh(x, power_vals, color=colors, alpha=0.8, height=0.6)
ax2.set_yticks(x)
ax2.set_yticklabels(short_names, fontsize=8, color=text_color)
ax2.set_xlabel('Power (W)', color=text_color, fontsize=10)
for i, (pw, cost) in enumerate(zip(power_vals, cost_vals)):
    ax2.text(pw + 5, i, f'{pw:.0f}W | £{cost:.2f}/day',
             va='center', fontsize=8, color=text_color)

# ── Panel 3: Monthly Cost Comparison ──
ax3 = fig.add_subplot(gs[0, 2])
style_ax(ax3, 'Monthly Electricity Cost (£)')
monthly = [models[n]["cost_month_gbp"] for n in names]
bars3 = ax3.barh(x, monthly, color=colors, alpha=0.8, height=0.6)
ax3.set_yticks(x)
ax3.set_yticklabels(short_names, fontsize=8, color=text_color)
ax3.set_xlabel('£/month', color=text_color, fontsize=10)
for i, v in enumerate(monthly):
    ax3.text(v + 0.1, i, f'£{v:.2f}', va='center', fontsize=9, color=text_color)

# ── Panel 4: Radar Chart — Category Scores ──
ax4 = fig.add_subplot(gs[1, 0], polar=True)
ax4.set_facecolor(panel_bg)
ax4.set_title('Category Radar', color='white', fontsize=13, fontweight='bold', pad=20)

categories = list(list(models.values())[0]["categories"].keys())
N = len(categories)
angles = np.linspace(0, 2 * np.pi, N, endpoint=False).tolist()
angles += angles[:1]

for name, m in models.items():
    vals = [m["categories"][c] for c in categories]
    vals += vals[:1]
    ax4.plot(angles, vals, color=m["color"], linewidth=1.5, alpha=0.8)
    ax4.fill(angles, vals, color=m["color"], alpha=0.08)

ax4.set_xticks(angles[:-1])
ax4.set_xticklabels(categories, fontsize=7, color=text_color)
ax4.set_ylim(0, 110)
ax4.set_yticks([25, 50, 75, 100])
ax4.set_yticklabels(['25%', '50%', '75%', '100%'], fontsize=7, color='#484f58')
ax4.tick_params(axis='y', colors='#484f58')
ax4.spines['polar'].set_color(grid_color)
ax4.grid(color=grid_color, alpha=0.3)

# ── Panel 5: Efficiency (quality*speed/power) ──
ax5 = fig.add_subplot(gs[1, 1])
style_ax(ax5, 'Efficiency Score (quality × speed / power)')
eff_vals = [models[n]["efficiency"] for n in names]
bars5 = ax5.barh(x, eff_vals, color=colors, alpha=0.8, height=0.6)
ax5.set_yticks(x)
ax5.set_yticklabels(short_names, fontsize=8, color=text_color)
ax5.set_xlabel('Efficiency Index', color=text_color, fontsize=10)
# Highlight winner
winner_idx = names.index(winner[0])
bars5[winner_idx].set_edgecolor('#ffd700')
bars5[winner_idx].set_linewidth(3)
for i, v in enumerate(eff_vals):
    ax5.text(v + 0.2, i, f'{v:.1f}', va='center', fontsize=9, color=text_color)

# ── Panel 6: Composite Winner Score ──
ax6 = fig.add_subplot(gs[1, 2])
style_ax(ax6, 'Composite Score (40% quality, 25% speed, 20% eff, 15% VRAM)')
composite_vals = [models[n]["composite"] for n in names]
sorted_idx = np.argsort(composite_vals)
sorted_names = [short_names[i] for i in sorted_idx]
sorted_vals = [composite_vals[i] for i in sorted_idx]
sorted_colors = [colors[i] for i in sorted_idx]

bars6 = ax6.barh(range(len(names)), sorted_vals, color=sorted_colors, alpha=0.85, height=0.6)
ax6.set_yticks(range(len(names)))
ax6.set_yticklabels(sorted_names, fontsize=8, color=text_color)
ax6.set_xlabel('Composite Index', color=text_color, fontsize=10)
# Gold border on winner
bars6[-1].set_edgecolor('#ffd700')
bars6[-1].set_linewidth(3)
for i, v in enumerate(sorted_vals):
    label = f'{v:.3f}'
    if sorted_names[i] == short_names[winner_idx]:
        label += ' ★ WINNER'
    ax6.text(v + 0.005, i, label, va='center', fontsize=9,
             color='#ffd700' if i == len(names)-1 else text_color)

# ── Panel 7: Speed Comparison (Gen + Prompt) ──
ax7 = fig.add_subplot(gs[2, 0])
style_ax(ax7, 'Generation & Prompt Speed')
gen_speeds = [models[n]["gen_speed"] for n in names]
prompt_speeds = [models[n]["prompt_speed"] for n in names]
width = 0.35
ax7.barh(x - width/2, gen_speeds, width, color=colors, alpha=0.9, label='Gen (t/s)')
ax7.barh(x + width/2, [p/10 for p in prompt_speeds], width, color=colors, alpha=0.4,
         label='Prompt (t/s ÷10)')
ax7.set_yticks(x)
ax7.set_yticklabels(short_names, fontsize=8, color=text_color)
ax7.set_xlabel('Tokens/sec', color=text_color, fontsize=10)
ax7.legend(fontsize=8, facecolor=panel_bg, edgecolor=grid_color, labelcolor=text_color)
for i, (g, p) in enumerate(zip(gen_speeds, prompt_speeds)):
    ax7.text(max(g, p/10) + 2, i, f'{g:.0f} / {p:.0f}', va='center', fontsize=8, color=text_color)

# ── Panel 8: VRAM & Disk Usage ──
ax8 = fig.add_subplot(gs[2, 1])
style_ax(ax8, 'VRAM & Disk Usage (GB)')
vram_vals = [models[n]["vram_gb"] for n in names]
disk_vals = [models[n]["disk_gb"] for n in names]
ax8.barh(x - width/2, vram_vals, width, color=colors, alpha=0.9, label='VRAM (GB)')
ax8.barh(x + width/2, disk_vals, width, color=colors, alpha=0.4, label='Disk (GB)')
ax8.axvline(x=98, color='#f85149', linestyle='--', alpha=0.5, label='98GB VRAM limit')
ax8.set_yticks(x)
ax8.set_yticklabels(short_names, fontsize=8, color=text_color)
ax8.set_xlabel('GB', color=text_color, fontsize=10)
ax8.legend(fontsize=8, facecolor=panel_bg, edgecolor=grid_color, labelcolor=text_color)

# ── Panel 9: Summary Table ──
ax9 = fig.add_subplot(gs[2, 2])
ax9.set_facecolor(panel_bg)
ax9.axis('off')
ax9.set_title('Summary', color='white', fontsize=13, fontweight='bold', pad=10)

summary_text = f"""
★ WINNER: {winner[0].replace(chr(10), ' ')}
  Composite: {winner[1]['composite']:.3f}
  Quality: {winner[1]['pct']:.1f}%  |  Gen: {winner[1]['gen_speed']:.0f} t/s
  Power: {winner[1]['power_w']:.0f}W  |  Cost: £{winner[1]['cost_month_gbp']:.2f}/mo
  VRAM: {winner[1]['vram_gb']}GB  |  Disk: {winner[1]['disk_gb']}GB

Key Insights:
• BF16 MoE matches 120B quality at 1.4x speed
• APEX-IQ is 6x faster than dense 31B
• Qwen 122B worst quality despite being largest
• MoE architecture dominates efficiency
• All models: £{min(m['cost_month_gbp'] for m in models.values()):.2f}-£{max(m['cost_month_gbp'] for m in models.values()):.2f}/month
"""

ax9.text(0.05, 0.95, summary_text, transform=ax9.transAxes,
         fontsize=10, color=text_color, verticalalignment='top',
         fontfamily='monospace',
         bbox=dict(boxstyle='round,pad=0.5', facecolor='#0d1117', edgecolor='#30363d'))

# Save
outpath = '/home/john/githubs/llm-server/logs/model-comparison.png'
plt.savefig(outpath, dpi=150, facecolor=dark_bg, edgecolor='none')
print(f"Saved to {outpath}")
plt.close()
