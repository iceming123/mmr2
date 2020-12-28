#!/bin/python
import matplotlib.pyplot as plt
import matplotlib
import pandas as pd
import numpy as np
import matplotlib     

measurements = pd.read_csv('measurementsContinuation.csv')

df = pd.DataFrame(measurements)

fig, axes = plt.subplots(3,1)

df.plot(ax=axes[0], x='block_number', y='complete_validation_time', legend=False, color='r')
df.plot(ax=axes[1], x='block_number', y='complete_proof_size', legend=False)
df.plot(ax=axes[2], x='block_number', y='required_blocks', legend=False)
df.plot(ax=axes[2], x='block_number', y='epoch_numbers', legend=False)

axes[0].set_ylabel('Seconds', fontsize=20)
axes[0].set_xlabel('Block number', fontsize=20)
axes[0].set_xlim(6350000,7000000)
axes[0].set_ylim(0, 600)
axes[0].tick_params(labelsize=15)
axes[0].yaxis.grid(True)
axes[0].xaxis.grid(True)

patches, labels = axes[0].get_legend_handles_labels()
axes[0].legend(patches, ["Complete Validation Time"], loc='upper right', fontsize=15)

axes[1].set_ylabel('Kilobyte', fontsize=20)
axes[1].set_xlabel('Block number', fontsize=20)
axes[1].set_xlim(6350000,7000000)
axes[1].set_ylim(0, 900)
axes[1].tick_params(labelsize=15)
axes[1].yaxis.grid(True)
axes[1].xaxis.grid(True)

patches, labels = axes[1].get_legend_handles_labels()
axes[1].legend(patches, ["Proof Size"], loc='upper right', fontsize=15)


axes[2].set_ylabel('Number', fontsize=20)
axes[2].set_xlabel('Block number', fontsize=20)
axes[2].set_xlim(6350000,7000000)
axes[2].set_ylim(0, 1100)
axes[2].tick_params(labelsize=15)
axes[2].yaxis.grid(True)
axes[2].xaxis.grid(True)

patches, labels = axes[2].get_legend_handles_labels()
axes[2].legend(patches, ["Required blocks", "Epoch numbers"], loc='upper right', fontsize=15)


plt.show()
