plugins {
	kotlin("jvm") version "{{ kotlin_version }}"
	kotlin("plugin.spring") version "{{ kotlin_version }}"
	id("org.springframework.boot") version "{{ boot_version }}"
	id("io.spring.dependency-management") version "1.1.7"
{%- if has_jpa %}
	kotlin("plugin.jpa") version "{{ kotlin_version }}"
{%- endif %}
}

group = "{{ group_id }}"
version = "0.0.1-SNAPSHOT"
description = "{{ description }}"

java {
	toolchain {
		languageVersion = JavaLanguageVersion.of({{ java_version }})
	}
}

repositories {
	mavenCentral()
{%- for repo in repositories %}
	maven {
		url = uri("{{ repo.url }}")
	}
{%- endfor %}
}

dependencies {
{%- for dep in compile_deps %}
	implementation("{{ dep.group_id }}:{{ dep.artifact_id }}{% if dep.version %}:{{ dep.version }}{% endif %}")
{%- endfor %}
	implementation("org.jetbrains.kotlin:kotlin-reflect")
	implementation("tools.jackson.module:jackson-module-kotlin")
{%- for dep in runtime_deps %}
	runtimeOnly("{{ dep.group_id }}:{{ dep.artifact_id }}{% if dep.version %}:{{ dep.version }}{% endif %}")
{%- endfor %}
{%- for dep in annotation_processor_deps %}
	annotationProcessor("{{ dep.group_id }}:{{ dep.artifact_id }}{% if dep.version %}:{{ dep.version }}{% endif %}")
{%- endfor %}
{%- for dep in test_deps %}
	testImplementation("{{ dep.group_id }}:{{ dep.artifact_id }}{% if dep.version %}:{{ dep.version }}{% endif %}")
{%- endfor %}
	testImplementation("org.jetbrains.kotlin:kotlin-test-junit5")
	testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}
{%- if boms %}

dependencyManagement {
	imports {
{%- for bom in boms %}
		mavenBom("{{ bom.group_id }}:{{ bom.artifact_id }}:{{ bom.version }}")
{%- endfor %}
	}
}
{%- endif %}

kotlin {
	compilerOptions {
		freeCompilerArgs.addAll("-Xjsr305=strict", "-Xannotation-default-target=param-property")
	}
}
{%- if has_jpa %}

allOpen {
	annotation("jakarta.persistence.Entity")
	annotation("jakarta.persistence.MappedSuperclass")
	annotation("jakarta.persistence.Embeddable")
}
{%- endif %}

tasks.withType<Test> {
	useJUnitPlatform()
}
