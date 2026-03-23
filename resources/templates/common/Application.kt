package {{ package_name }}

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication

@SpringBootApplication
class {{ application_name }}

fun main(args: Array<String>) {
	runApplication<{{ application_name }}>(*args)
}
