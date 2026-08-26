<!-- Poolside water background: gradient + mesh color field + caustics (displacement-refracted)
     + glow blobs + koi. All transform-only animations; no animated filters on moving nodes. -->
<div class="ps-water" aria-hidden="true">
	<div class="ps-mesh" aria-hidden="true"></div>
	<div class="ps-caustic-wrap">
		<div class="ps-caustic c1"></div>
		<div class="ps-caustic c2"></div>
	</div>
	<div class="ps-blob coral"></div>
	<div class="ps-blob sky"></div>

	<!-- koi 1: orange-and-white butterfly koi -->
	<svg class="ps-koi k1" viewBox="0 0 200 90">
		<defs>
			<radialGradient id="kb1" cx="35%" cy="40%" r="70%">
				<stop offset="0%" stop-color="#FFF8F0"/><stop offset="45%" stop-color="#F5E6D0"/>
				<stop offset="72%" stop-color="#F28C3B"/><stop offset="100%" stop-color="#D96A1F"/>
			</radialGradient>
			<linearGradient id="kt1" x1="0" y1="0" x2="1" y2="0">
				<stop offset="0%" stop-color="#E86A1F" stop-opacity=".9"/><stop offset="100%" stop-color="#F28C3B" stop-opacity=".4"/>
			</linearGradient>
		</defs>
		<!-- tail -->
		<path d="M158 45 C172 30 188 22 198 18 C192 32 190 40 192 48 C194 56 190 62 184 66 C176 60 166 52 158 45 Z" fill="url(#kt1)" opacity=".85"/>
		<!-- body -->
		<ellipse cx="85" cy="45" rx="78" ry="28" fill="url(#kb1)"/>
		<!-- head patch -->
		<ellipse cx="32" cy="40" rx="20" ry="14" fill="#E86A1F" opacity=".55"/>
		<!-- mid patch -->
		<ellipse cx="90" cy="42" rx="28" ry="16" fill="#D96A1F" opacity=".35"/>
		<!-- pectoral fins -->
		<path d="M52 62 C46 74 36 80 26 80 C32 70 36 62 42 56 Z" fill="#F5E6D0" opacity=".7"/>
		<path d="M118 62 C114 74 106 80 98 80 C104 70 110 62 114 56 Z" fill="#F5E6D0" opacity=".6"/>
		<!-- dorsal hint -->
		<path d="M60 18 C80 12 110 12 130 18 C118 14 80 14 60 18 Z" fill="#FFF8F0" opacity=".5"/>
		<!-- eye -->
		<circle cx="18" cy="38" r="3.2" fill="#1a1a1a"/>
		<circle cx="17.2" cy="37.2" r="1" fill="#fff"/>
	</svg>

	<!-- koi 2: white-and-red kohaku -->
	<svg class="ps-koi k2" viewBox="0 0 200 90">
		<defs>
			<radialGradient id="kb2" cx="40%" cy="38%" r="70%">
				<stop offset="0%" stop-color="#FFFFFF"/><stop offset="60%" stop-color="#F7F1E6"/>
				<stop offset="100%" stop-color="#E8DFC8"/>
			</radialGradient>
			<linearGradient id="kt2" x1="0" y1="0" x2="1" y2="0">
				<stop offset="0%" stop-color="#D9CFBA" stop-opacity=".9"/><stop offset="100%" stop-color="#F7F1E6" stop-opacity=".4"/>
			</linearGradient>
		</defs>
		<path d="M158 45 C172 30 188 22 198 18 C192 32 190 40 192 48 C194 56 190 62 184 66 C176 60 166 52 158 45 Z" fill="url(#kt2)" opacity=".85"/>
		<ellipse cx="85" cy="45" rx="78" ry="28" fill="url(#kb2)"/>
		<ellipse cx="48" cy="38" rx="18" ry="12" fill="#E02020" opacity=".5"/>
		<ellipse cx="100" cy="48" rx="22" ry="13" fill="#E02020" opacity=".38"/>
		<path d="M52 62 C46 74 36 80 26 80 C32 70 36 62 42 56 Z" fill="#F7F1E6" opacity=".7"/>
		<path d="M118 62 C114 74 106 80 98 80 C104 70 110 62 114 56 Z" fill="#F7F1E6" opacity=".6"/>
		<circle cx="18" cy="38" r="3.2" fill="#1a1a1a"/>
		<circle cx="17.2" cy="37.2" r="1" fill="#fff"/>
	</svg>
</div>
