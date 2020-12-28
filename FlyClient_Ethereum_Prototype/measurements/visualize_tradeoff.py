#!/bin/python
import matplotlib.pyplot as plt
import matplotlib
import pandas as pd
import numpy as np
import matplotlib     

proof_sizes = pd.read_csv('measurementsTradeoff.csv')

df = pd.DataFrame(proof_sizes)

fig = plt.figure()
ax1 = fig.add_subplot(111)
ax2 = ax1.twinx()

df.plot(ax=ax1, x='l', y='complete_validation_time', legend=False, color='r')
df.plot(ax=ax2, x='l', y='complete_proof_size', legend=False)
ax1.set_ylabel('Seconds', fontsize=20)
ax2.set_ylabel('Kilobyte', fontsize=20)
ax1.set_xlabel('L', fontsize=20)
ax1.set_xlim(100,1000)
ax1.set_ylim(0, 800)
ax2.set_ylim(0, 1200)

patches, labels = ax1.get_legend_handles_labels()
ax1.legend(patches, ["Complete Validation Time"], loc='upper left', fontsize=15)

patches, labels = ax2.get_legend_handles_labels()
ax2.legend(patches, ["Proof Size"], loc='upper right', fontsize=15)

ax1.tick_params(labelsize=15)
ax2.tick_params(labelsize=15)

ax1.yaxis.grid(True)
ax1.xaxis.grid(True)

plt.show()
